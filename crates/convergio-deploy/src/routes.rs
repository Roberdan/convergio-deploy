//! HTTP routes for deploy extension.

use crate::schema::DeployDb;
use crate::types::PushStrategy;
use crate::validation;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;

pub(crate) type SharedDb = Arc<DeployDb>;

pub fn deploy_routes(db: SharedDb) -> Router {
    Router::new()
        .route("/api/deploy/upgrade", post(upgrade_handler))
        .route("/api/deploy/push-all", post(push_all_handler))
        .route("/api/deploy/status", get(status_handler))
        .route("/api/deploy/history", get(history_handler))
        .route("/api/deploy/diagnostics", post(diagnostics_handler))
        .route(
            "/api/deploy/diagnostics/report-issue",
            post(report_issue_handler),
        )
        .with_state(db)
}

#[derive(serde::Deserialize)]
struct UpgradeRequest {
    version: Option<String>,
}

async fn upgrade_handler(
    State(db): State<SharedDb>,
    Json(req): Json<UpgradeRequest>,
) -> Json<serde_json::Value> {
    // Validate version tag if provided
    if let Some(ref v) = req.version {
        if let Err(e) = validation::validate_version_tag(v) {
            return Json(serde_json::json!({ "ok": false, "error": e }));
        }
    }

    let result = crate::upgrader::upgrade(&db, req.version.as_deref()).await;
    match result {
        Ok(record) => Json(serde_json::json!({
            "ok": true,
            "upgrade": record,
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e,
        })),
    }
}

#[derive(serde::Deserialize)]
struct PushAllRequest {
    version: String,
    peers: Vec<String>,
    #[serde(default = "default_strategy")]
    strategy: PushStrategy,
}

fn default_strategy() -> PushStrategy {
    PushStrategy::Rolling
}

async fn push_all_handler(
    State(db): State<SharedDb>,
    Json(req): Json<PushAllRequest>,
) -> Json<serde_json::Value> {
    // Validate version
    if let Err(e) = validation::validate_version_tag(&req.version) {
        return Json(serde_json::json!({ "ok": false, "error": e }));
    }

    // Validate peer URLs (SSRF protection)
    if let Err(e) = validation::validate_peers(&req.peers) {
        return Json(serde_json::json!({ "ok": false, "error": e }));
    }

    // Require auth token — refuse to push with empty credentials
    let token = std::env::var("CONVERGIO_AUTH_TOKEN").unwrap_or_default();
    if token.is_empty() {
        return Json(serde_json::json!({
            "ok": false,
            "error": "CONVERGIO_AUTH_TOKEN not set — cannot authenticate to peers",
        }));
    }

    let result =
        crate::push_all::push_all(&db, &req.peers, &req.version, req.strategy, &token).await;
    match result {
        Ok(job) => Json(serde_json::json!({
            "ok": true,
            "job": job,
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e,
        })),
    }
}

async fn status_handler(State(_db): State<SharedDb>) -> Json<serde_json::Value> {
    let current_version = env!("CARGO_PKG_VERSION");
    let platform = crate::types::detect_platform();
    Json(serde_json::json!({
        "version": current_version,
        "platform": platform,
    }))
}

async fn history_handler(State(db): State<SharedDb>) -> Json<serde_json::Value> {
    let records = db.list_upgrades(50);
    Json(serde_json::json!({
        "count": records.len(),
        "upgrades": records,
    }))
}

async fn diagnostics_handler(State(db): State<SharedDb>) -> Json<serde_json::Value> {
    let report = crate::diagnostics::run_diagnostics(db.pool()).await;
    Json(serde_json::json!({
        "ok": report.failed == 0,
        "report": report,
    }))
}

#[derive(serde::Deserialize)]
struct ReportIssueRequest {
    gh_token: String,
}

async fn report_issue_handler(
    State(db): State<SharedDb>,
    Json(req): Json<ReportIssueRequest>,
) -> Json<serde_json::Value> {
    // Validate token is non-empty (don't log it)
    if req.gh_token.is_empty() {
        return Json(serde_json::json!({
            "ok": false,
            "error": "gh_token is required",
        }));
    }

    let report = crate::diagnostics::run_diagnostics(db.pool()).await;
    match crate::diagnostics::create_github_issue(&report, &req.gh_token).await {
        Ok(url) => Json(serde_json::json!({
            "ok": true,
            "issue_url": url,
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e,
        })),
    }
}
