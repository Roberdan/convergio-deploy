//! Self-upgrade: download, verify, backup, replace, restart.

use crate::github;
use crate::schema::DeployDb;
use crate::types::{UpgradeRecord, UpgradeStatus};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Run the self-upgrade flow for the current node.
pub async fn upgrade(db: &DeployDb, target_tag: Option<&str>) -> Result<UpgradeRecord, String> {
    let release = match target_tag {
        Some(tag) => github::fetch_release_by_tag(tag).await?,
        None => github::fetch_latest_release().await?,
    };

    let platform = crate::types::detect_platform();
    let asset = github::find_platform_asset(&release, &platform)
        .ok_or_else(|| format!("No asset for platform {platform}"))?;

    let current_version = env!("CARGO_PKG_VERSION");
    let target_version = release.tag.trim_start_matches('v');

    if current_version == target_version {
        return Err(format!("Already at version {current_version}"));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let mut record = UpgradeRecord {
        id: id.clone(),
        from_version: current_version.to_string(),
        to_version: target_version.to_string(),
        status: UpgradeStatus::Downloading,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
        error: None,
        backup_path: None,
    };
    db.insert_upgrade(&record);

    // 1. Download
    tracing::info!(version = target_version, "downloading upgrade");
    let bytes = github::download_asset(asset).await.inspect_err(|e| {
        record.status = UpgradeStatus::Failed;
        record.error = Some(e.clone());
        db.update_upgrade(&record);
    })?;

    // 2. Verify SHA256
    record.status = UpgradeStatus::Verifying;
    db.update_upgrade(&record);

    if let Ok(checksums) = github::fetch_checksums(&release).await {
        let actual_hash = hex_sha256(&bytes);
        if !checksums.contains(&actual_hash) {
            let msg = "SHA256 mismatch — download corrupted".to_string();
            record.status = UpgradeStatus::Failed;
            record.error = Some(msg.clone());
            db.update_upgrade(&record);
            return Err(msg);
        }
        tracing::info!("SHA256 verified");
    } else {
        tracing::warn!("No checksum file in release — integrity cannot be verified");
    }

    // 3. Backup current binary
    record.status = UpgradeStatus::BackingUp;
    db.update_upgrade(&record);

    let current_exe =
        std::env::current_exe().map_err(|e| format!("Cannot find current exe: {e}"))?;
    let backup = backup_binary(&current_exe)?;
    record.backup_path = Some(backup.to_string_lossy().to_string());
    db.update_upgrade(&record);

    // 4. Extract and replace
    record.status = UpgradeStatus::Replacing;
    db.update_upgrade(&record);

    if let Err(e) = extract_and_replace(&bytes, &current_exe) {
        // Rollback
        tracing::error!("Replace failed, rolling back: {e}");
        if let Err(rb_err) = rollback(&backup, &current_exe) {
            tracing::error!("Rollback also failed: {rb_err}");
        }
        record.status = UpgradeStatus::RolledBack;
        record.error = Some(e.clone());
        record.completed_at = Some(chrono::Utc::now().to_rfc3339());
        db.update_upgrade(&record);
        return Err(e);
    }

    // 5. Restart
    record.status = UpgradeStatus::Restarting;
    db.update_upgrade(&record);

    let restart = convergio_types::platform_restart::restart_daemon();
    if !restart.success {
        tracing::warn!("Auto-restart failed: {}", restart.message);
    }

    record.status = UpgradeStatus::Completed;
    record.completed_at = Some(chrono::Utc::now().to_rfc3339());
    db.update_upgrade(&record);

    tracing::info!(
        from = current_version,
        to = target_version,
        "upgrade completed"
    );
    Ok(record)
}

/// Rollback to backup binary.
pub fn rollback(backup: &Path, target: &Path) -> Result<(), String> {
    std::fs::copy(backup, target)
        .map(|_| ())
        .map_err(|e| format!("Rollback copy failed: {e}"))
}

fn backup_binary(exe: &Path) -> Result<PathBuf, String> {
    let backup_dir = convergio_types::platform_paths::convergio_data_dir().join("backups");
    std::fs::create_dir_all(&backup_dir).map_err(|e| format!("Cannot create backup dir: {e}"))?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_name = format!("convergio.{timestamp}.bak");
    let backup_path = backup_dir.join(backup_name);

    std::fs::copy(exe, &backup_path).map_err(|e| format!("Backup copy failed: {e}"))?;

    tracing::info!(path = %backup_path.display(), "binary backed up");
    Ok(backup_path)
}

fn extract_and_replace(archive_bytes: &[u8], target: &Path) -> Result<(), String> {
    let data_dir = convergio_types::platform_paths::convergio_data_dir();
    let temp_dir = data_dir.join("upgrade-staging");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Cannot create staging dir: {e}"))?;

    let archive_path = temp_dir.join("archive");
    std::fs::write(&archive_path, archive_bytes)
        .map_err(|e| format!("Write archive failed: {e}"))?;

    let binary_name = if cfg!(target_os = "windows") {
        "convergio.exe"
    } else {
        "convergio"
    };

    let extracted = temp_dir.join(binary_name);

    // Attempt tar extraction with --strip-components to flatten
    let tar_result = std::process::Command::new("tar")
        .args([
            "xzf",
            &archive_path.to_string_lossy(),
            "--strip-components=0",
            "-C",
        ])
        .arg(&temp_dir)
        .output();

    if tar_result.is_ok() && extracted.exists() {
        // Validate extracted binary is within temp_dir (path traversal check)
        let canonical_temp = temp_dir
            .canonicalize()
            .map_err(|e| format!("Cannot canonicalize temp dir: {e}"))?;
        let canonical_extracted = extracted
            .canonicalize()
            .map_err(|e| format!("Cannot canonicalize extracted: {e}"))?;
        if !canonical_extracted.starts_with(&canonical_temp) {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err("Path traversal detected in archive — aborting".into());
        }
    } else {
        // Maybe it's a raw binary
        std::fs::rename(&archive_path, &extracted)
            .map_err(|e| format!("Cannot move binary: {e}"))?;
    }

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&extracted, perms).map_err(|e| format!("chmod failed: {e}"))?;
    }

    // Replace target binary
    std::fs::copy(&extracted, target).map_err(|e| format!("Replace binary failed: {e}"))?;

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
