//! MCP tool definitions for the deploy extension.

use convergio_types::extension::McpToolDef;
use serde_json::json;

pub fn deploy_tools() -> Vec<McpToolDef> {
    vec![
        McpToolDef {
            name: "cvg_deploy_status".into(),
            description: "Get current deployment status.".into(),
            method: "GET".into(),
            path: "/api/deploy/status".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "community".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_deploy_history".into(),
            description: "Get deployment history.".into(),
            method: "GET".into(),
            path: "/api/deploy/history".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "community".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_deploy_diagnostics".into(),
            description: "Run deployment diagnostics.".into(),
            method: "GET".into(),
            path: "/api/deploy/diagnostics".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "community".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_deploy_report_issue".into(),
            description: "Report a deployment issue.".into(),
            method: "POST".into(),
            path: "/api/deploy/diagnostics/report-issue".into(),
            input_schema: json!({"type": "object", "properties": {"description": {"type": "string"}}, "required": ["description"]}),
            min_ring: "trusted".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_build_status".into(),
            description: "Get status of a build.".into(),
            method: "GET".into(),
            path: "/api/build/status/:id".into(),
            input_schema: json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
            min_ring: "community".into(),
            path_params: vec!["id".into()],
        },
        McpToolDef {
            name: "cvg_build_history".into(),
            description: "Get build history.".into(),
            method: "GET".into(),
            path: "/api/build/history".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "community".into(),
            path_params: vec![],
        },
    ]
}
