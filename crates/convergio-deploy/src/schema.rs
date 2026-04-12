//! Deploy DB schema and queries.

use crate::types::{PushAllJob, UpgradeRecord};
use convergio_db::pool::ConnPool;

pub struct DeployDb {
    pool: ConnPool,
}

impl DeployDb {
    pub fn new(pool: ConnPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &ConnPool {
        &self.pool
    }

    pub fn insert_upgrade(&self, record: &UpgradeRecord) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("DB error: {e}");
                return;
            }
        };
        let _ = conn.execute(
            "INSERT INTO deploy_upgrades \
             (id, from_version, to_version, status, started_at, backup_path) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                record.id,
                record.from_version,
                record.to_version,
                record.status.to_string(),
                record.started_at,
                record.backup_path,
            ],
        );
    }

    pub fn update_upgrade(&self, record: &UpgradeRecord) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("DB error: {e}");
                return;
            }
        };
        let _ = conn.execute(
            "UPDATE deploy_upgrades \
             SET status = ?1, completed_at = ?2, error = ?3 \
             WHERE id = ?4",
            rusqlite::params![
                record.status.to_string(),
                record.completed_at,
                record.error,
                record.id,
            ],
        );
    }

    pub fn list_upgrades(&self, limit: u32) -> Vec<UpgradeRecord> {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let mut stmt = match conn.prepare(
            "SELECT id, from_version, to_version, status, \
             started_at, completed_at, error, backup_path \
             FROM deploy_upgrades ORDER BY started_at DESC LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([limit], |row| {
            Ok(UpgradeRecord {
                id: row.get(0)?,
                from_version: row.get(1)?,
                to_version: row.get(2)?,
                status: serde_json::from_value(serde_json::Value::String(row.get::<_, String>(3)?))
                    .unwrap_or(crate::types::UpgradeStatus::Failed),
                started_at: row.get(4)?,
                completed_at: row.get(5)?,
                error: row.get(6)?,
                backup_path: row.get(7)?,
            })
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn insert_push_job(&self, job: &PushAllJob) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("DB error: {e}");
                return;
            }
        };
        let peers_json = serde_json::to_string(&job.peers).unwrap_or_default();
        let _ = conn.execute(
            "INSERT INTO deploy_push_jobs \
             (id, version, strategy, peers_json, started_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                job.id,
                job.version,
                serde_json::to_string(&job.strategy).unwrap_or_default(),
                peers_json,
                job.started_at,
            ],
        );
    }

    pub fn update_push_job(&self, job: &PushAllJob) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("DB error: {e}");
                return;
            }
        };
        let peers_json = serde_json::to_string(&job.peers).unwrap_or_default();
        let _ = conn.execute(
            "UPDATE deploy_push_jobs SET peers_json = ?1 WHERE id = ?2",
            rusqlite::params![peers_json, job.id],
        );
    }
}
