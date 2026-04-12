//! Push-all: upgrade every mesh peer (rolling or parallel).

use crate::schema::DeployDb;
use crate::types::{PeerUpgradeStatus, PushAllJob, PushStrategy, UpgradeStatus};
use reqwest::Client;

/// Start a push-all job: trigger upgrade on all mesh peers.
pub async fn push_all(
    db: &DeployDb,
    peers: &[String],
    version: &str,
    strategy: PushStrategy,
    auth_token: &str,
) -> Result<PushAllJob, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let peer_statuses: Vec<PeerUpgradeStatus> = peers
        .iter()
        .map(|p| PeerUpgradeStatus {
            peer_url: p.clone(),
            status: UpgradeStatus::Downloading,
            error: None,
        })
        .collect();

    let job = PushAllJob {
        id: id.clone(),
        version: version.to_string(),
        strategy: strategy.clone(),
        peers: peer_statuses,
        started_at: chrono::Utc::now().to_rfc3339(),
    };
    db.insert_push_job(&job);

    match strategy {
        PushStrategy::Rolling => push_rolling(db, &job, auth_token).await,
        PushStrategy::Parallel => push_parallel(db, &job, auth_token).await,
    }
}

async fn push_rolling(
    db: &DeployDb,
    job: &PushAllJob,
    auth_token: &str,
) -> Result<PushAllJob, String> {
    let mut updated = job.clone();
    for (i, peer) in job.peers.iter().enumerate() {
        let result = trigger_peer_upgrade(&peer.peer_url, &job.version, auth_token).await;
        updated.peers[i] = match result {
            Ok(()) => PeerUpgradeStatus {
                peer_url: peer.peer_url.clone(),
                status: UpgradeStatus::Completed,
                error: None,
            },
            Err(e) => {
                tracing::warn!(peer = %peer.peer_url, error = %e, "peer upgrade failed");
                PeerUpgradeStatus {
                    peer_url: peer.peer_url.clone(),
                    status: UpgradeStatus::Failed,
                    error: Some(e),
                }
            }
        };
        db.update_push_job(&updated);
    }
    Ok(updated)
}

async fn push_parallel(
    db: &DeployDb,
    job: &PushAllJob,
    auth_token: &str,
) -> Result<PushAllJob, String> {
    let mut handles = Vec::new();
    for peer in &job.peers {
        let url = peer.peer_url.clone();
        let ver = job.version.clone();
        let tok = auth_token.to_string();
        handles.push(tokio::spawn(async move {
            (url.clone(), trigger_peer_upgrade(&url, &ver, &tok).await)
        }));
    }

    let mut updated = job.clone();
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.await {
            Ok((_, Ok(()))) => {
                updated.peers[i].status = UpgradeStatus::Completed;
            }
            Ok((_, Err(e))) => {
                updated.peers[i].status = UpgradeStatus::Failed;
                updated.peers[i].error = Some(e);
            }
            Err(e) => {
                updated.peers[i].status = UpgradeStatus::Failed;
                updated.peers[i].error = Some(format!("Task panicked: {e}"));
            }
        }
    }
    db.update_push_job(&updated);
    Ok(updated)
}

async fn trigger_peer_upgrade(
    peer_url: &str,
    version: &str,
    auth_token: &str,
) -> Result<(), String> {
    // Validate peer URL before making any request (defense-in-depth)
    crate::validation::validate_peer_url(peer_url)?;

    let url = format!("{peer_url}/api/deploy/upgrade");
    let client = Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {auth_token}"))
        .json(&serde_json::json!({ "version": version }))
        .send()
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // Truncate response body to prevent log injection
        let body_truncated = if body.len() > 500 {
            format!("{}...[truncated]", &body[..500])
        } else {
            body
        };
        Err(format!("Peer returned HTTP {status}: {body_truncated}"))
    }
}
