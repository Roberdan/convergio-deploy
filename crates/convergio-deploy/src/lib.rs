//! convergio-deploy — self-upgrade, push-all, fleet deployment.

pub mod diagnostics;
pub mod ext;
pub mod github;
pub mod push_all;
mod routes;
pub mod schema;
pub mod types;
pub mod update_check;
pub mod upgrader;
pub mod validation;

pub use ext::DeployExtension;
pub mod mcp_defs;
