//! Periodic update check — notifies user when a new version is available.
//! Runs every 6 hours via scheduled_tasks. Does NOT auto-upgrade.

use crate::github;
use convergio_db::pool::ConnPool;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check GitHub for new releases and log/store notification.
pub async fn check_and_notify(pool: ConnPool) -> Result<(), String> {
    let release = github::fetch_latest_release().await?;
    let latest = release.tag.trim_start_matches('v');

    if latest == CURRENT_VERSION {
        tracing::debug!("Up to date: v{CURRENT_VERSION}");
        return Ok(());
    }

    // Compare versions (simple string compare — semver would be better)
    if latest > CURRENT_VERSION {
        tracing::info!(
            current = CURRENT_VERSION,
            available = latest,
            "New version available! Run `cvg deploy upgrade` to update."
        );

        // Store notification in DB for cockpit display
        if let Ok(conn) = pool.get() {
            if let Err(e) = conn.execute(
                "INSERT OR REPLACE INTO deploy_notifications \
                 (id, current_version, available_version, checked_at) \
                 VALUES ('latest', ?1, ?2, datetime('now'))",
                rusqlite::params![CURRENT_VERSION, latest],
            ) {
                tracing::warn!("Failed to store update notification: {e}");
            }
        }
    }

    Ok(())
}

/// Get the latest update notification (for CLI/cockpit display).
pub fn get_update_notification(pool: &ConnPool) -> Option<UpdateNotification> {
    let conn = pool.get().ok()?;
    conn.query_row(
        "SELECT current_version, available_version, checked_at \
         FROM deploy_notifications WHERE id = 'latest'",
        [],
        |row| {
            Ok(UpdateNotification {
                current: row.get(0)?,
                available: row.get(1)?,
                checked_at: row.get(2)?,
            })
        },
    )
    .ok()
    .filter(|n| n.available != n.current)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateNotification {
    pub current: String,
    pub available: String,
    pub checked_at: String,
}
