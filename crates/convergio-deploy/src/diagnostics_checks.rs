//! Diagnostic check functions for health, endpoints, network, and deploy.

use reqwest::Client;

use super::{DiagCheck, DiagReport};
use convergio_db::pool::ConnPool;

/// Run all diagnostic checks.
pub async fn run_diagnostics(pool: &ConnPool) -> DiagReport {
    let mut checks = Vec::new();
    checks.push(check_database(pool));
    checks.push(check_health_endpoint().await);
    checks.push(check_extension_routes().await);
    checks.push(check_disk_space());
    checks.push(check_config());
    checks.push(check_github_api().await);
    checks.push(check_deploy_status().await);

    let passed = checks.iter().filter(|c| c.passed).count();
    let failed = checks.iter().filter(|c| !c.passed).count();

    DiagReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: crate::types::detect_platform(),
        checks,
        passed,
        failed,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

pub(crate) fn check_database(pool: &ConnPool) -> DiagCheck {
    let start = std::time::Instant::now();
    match pool.get() {
        Ok(conn) => {
            let result = conn.query_row("SELECT 1", [], |_| Ok(()));
            let dur = start.elapsed().as_millis() as u64;
            match result {
                Ok(()) => DiagCheck {
                    name: "database".into(),
                    passed: true,
                    message: "SQLite connection OK".into(),
                    duration_ms: dur,
                },
                Err(e) => DiagCheck {
                    name: "database".into(),
                    passed: false,
                    message: format!("Query failed: {e}"),
                    duration_ms: dur,
                },
            }
        }
        Err(e) => DiagCheck {
            name: "database".into(),
            passed: false,
            message: format!("Pool error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

async fn check_health_endpoint() -> DiagCheck {
    let start = std::time::Instant::now();
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build();
    let client = match client {
        Ok(c) => c,
        Err(e) => {
            return DiagCheck {
                name: "health_endpoint".into(),
                passed: false,
                message: format!("Client build error: {e}"),
                duration_ms: 0,
            }
        }
    };

    match client.get("http://localhost:8420/api/health").send().await {
        Ok(resp) => DiagCheck {
            name: "health_endpoint".into(),
            passed: resp.status().is_success(),
            message: format!("HTTP {}", resp.status()),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => DiagCheck {
            name: "health_endpoint".into(),
            passed: false,
            message: format!("Connection failed: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

async fn check_extension_routes() -> DiagCheck {
    let start = std::time::Instant::now();
    let endpoints = ["/api/plans", "/api/agents", "/api/health/deep"];

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| Client::new());

    let mut failures = Vec::new();
    for ep in &endpoints {
        let url = format!("http://localhost:8420{ep}");
        match client.get(&url).send().await {
            Ok(r) if r.status().is_success() || r.status().as_u16() == 401 => {}
            Ok(r) => failures.push(format!("{ep}: HTTP {}", r.status())),
            Err(e) => failures.push(format!("{ep}: {e}")),
        }
    }

    DiagCheck {
        name: "extension_routes".into(),
        passed: failures.is_empty(),
        message: if failures.is_empty() {
            "All core routes responding".into()
        } else {
            format!("Failures: {}", failures.join(", "))
        },
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

fn check_disk_space() -> DiagCheck {
    let start = std::time::Instant::now();
    let data_dir = convergio_types::platform_paths::convergio_data_dir();
    let db_path = data_dir.join("convergio.db");

    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    let message = format!(
        "Data dir: {}, DB size: {} MB",
        sanitize_path(&data_dir),
        db_size / (1024 * 1024)
    );

    DiagCheck {
        name: "disk_space".into(),
        passed: true,
        message,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

fn check_config() -> DiagCheck {
    let start = std::time::Instant::now();
    let config_paths = [
        dirs::home_dir()
            .unwrap_or_default()
            .join(".convergio/config.toml"),
        convergio_types::platform_paths::convergio_data_dir().join("config.toml"),
    ];

    let found = config_paths.iter().find(|p| p.exists());
    match found {
        Some(path) => DiagCheck {
            name: "config".into(),
            passed: true,
            message: format!("Found: {}", sanitize_path(path)),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        None => DiagCheck {
            name: "config".into(),
            passed: false,
            message: "No config.toml found. Run `cvg setup`.".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// Replace home directory with ~ to avoid leaking usernames.
fn sanitize_path(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        return s.replace(home_str.as_ref(), "~");
    }
    s
}

async fn check_github_api() -> DiagCheck {
    let start = std::time::Instant::now();
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| Client::new());

    match client
        .get("https://api.github.com/repos/Roberdan/convergio/releases/latest")
        .header("User-Agent", "convergio-diag")
        .send()
        .await
    {
        Ok(r) => DiagCheck {
            name: "github_api".into(),
            passed: r.status().is_success(),
            message: format!("HTTP {} (rate limit OK)", r.status()),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => DiagCheck {
            name: "github_api".into(),
            passed: false,
            message: format!("Cannot reach GitHub: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

async fn check_deploy_status() -> DiagCheck {
    let start = std::time::Instant::now();
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| Client::new());

    match client
        .get("http://localhost:8420/api/deploy/status")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => DiagCheck {
            name: "deploy_status".into(),
            passed: true,
            message: "Deploy endpoint responding".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        _ => DiagCheck {
            name: "deploy_status".into(),
            passed: false,
            message: "Deploy endpoint not available".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}
