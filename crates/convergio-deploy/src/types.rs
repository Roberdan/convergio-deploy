//! Deploy types — shared across github, upgrader, push_all.

use serde::{Deserialize, Serialize};

/// A release asset from GitHub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: Option<String>,
}

/// A GitHub release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub tag: String,
    pub assets: Vec<ReleaseAsset>,
    pub published_at: String,
}

/// Upgrade status for tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeStatus {
    Downloading,
    Verifying,
    BackingUp,
    Replacing,
    Restarting,
    Completed,
    Failed,
    RolledBack,
}

impl std::fmt::Display for UpgradeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "unknown".into());
        f.write_str(&s)
    }
}

/// Record of an upgrade attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeRecord {
    pub id: String,
    pub from_version: String,
    pub to_version: String,
    pub status: UpgradeStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
    pub backup_path: Option<String>,
}

/// Push-all job tracking a fleet-wide upgrade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushAllJob {
    pub id: String,
    pub version: String,
    pub strategy: PushStrategy,
    pub peers: Vec<PeerUpgradeStatus>,
    pub started_at: String,
}

/// Rolling or parallel push strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PushStrategy {
    Rolling,
    Parallel,
}

/// Per-peer upgrade status in a push-all job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerUpgradeStatus {
    pub peer_url: String,
    pub status: UpgradeStatus,
    pub error: Option<String>,
}

/// Detect platform at runtime.
pub fn detect_platform() -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    match (arch, os) {
        ("aarch64", "macos") => "aarch64-apple-darwin".into(),
        ("x86_64", "macos") => "x86_64-apple-darwin".into(),
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu".into(),
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu".into(),
        ("x86_64", "windows") => "x86_64-pc-windows-msvc".into(),
        _ => format!("{arch}-{os}-unknown"),
    }
}
