//! Deploy extension — self-upgrade, push-all, deployment management.

use crate::routes;
use crate::schema::DeployDb;
use convergio_db::pool::ConnPool;
use convergio_types::extension::{
    AppContext, Extension, Health, McpToolDef, Migration, ScheduledTask,
};
use convergio_types::manifest::*;
use std::sync::Arc;

pub struct DeployExtension {
    pool: ConnPool,
}

impl DeployExtension {
    pub fn new(pool: ConnPool) -> Self {
        Self { pool }
    }
}

impl Extension for DeployExtension {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "convergio-deploy".into(),
            description: "Self-upgrade and fleet deployment".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            kind: ModuleKind::Platform,
            provides: vec![Capability {
                name: "deploy".into(),
                version: "1.0.0".into(),
                description: "Self-upgrade and push-all deployment".into(),
            }],
            requires: vec![Dependency {
                capability: "db-pool".into(),
                version_req: ">=1.0.0".into(),
                required: true,
            }],
            agent_tools: vec![],
            required_roles: vec!["orchestrator".into(), "all".into()],
        }
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![
            Migration {
                version: 1,
                description: "deploy tables",
                up: "CREATE TABLE IF NOT EXISTS deploy_upgrades (
                    id TEXT PRIMARY KEY,
                    from_version TEXT NOT NULL,
                    to_version TEXT NOT NULL,
                    status TEXT NOT NULL,
                    started_at TEXT NOT NULL,
                    completed_at TEXT,
                    error TEXT,
                    backup_path TEXT
                );
                CREATE TABLE IF NOT EXISTS deploy_push_jobs (
                    id TEXT PRIMARY KEY,
                    version TEXT NOT NULL,
                    strategy TEXT NOT NULL,
                    peers_json TEXT NOT NULL,
                    started_at TEXT NOT NULL
                );",
            },
            Migration {
                version: 2,
                description: "update notification table",
                up: "CREATE TABLE IF NOT EXISTS deploy_notifications (
                    id TEXT PRIMARY KEY,
                    current_version TEXT NOT NULL,
                    available_version TEXT NOT NULL,
                    checked_at TEXT NOT NULL
                );",
            },
        ]
    }

    fn routes(&self, _ctx: &AppContext) -> Option<axum::Router> {
        let db = Arc::new(DeployDb::new(self.pool.clone()));
        Some(routes::deploy_routes(db))
    }

    fn health(&self) -> Health {
        Health::Ok
    }

    fn scheduled_tasks(&self) -> Vec<ScheduledTask> {
        vec![ScheduledTask {
            name: "check-updates",
            cron: "0 */6 * * *", // every 6 hours
        }]
    }

    fn on_scheduled_task(&self, task_name: &str) {
        if task_name == "check-updates" {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::update_check::check_and_notify(pool).await {
                    tracing::warn!("Update check failed: {e}");
                }
            });
        }
    }

    fn mcp_tools(&self) -> Vec<McpToolDef> {
        crate::mcp_defs::deploy_tools()
    }
}
