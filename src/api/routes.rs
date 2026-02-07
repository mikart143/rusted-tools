use crate::api::handlers::ApiState;
use axum::{
    Router,
    routing::{get, post},
};

pub fn health_routes() -> Router<ApiState> {
    Router::new()
        .route("/health", get(super::handlers::health_check))
        .route("/info", get(super::handlers::server_info))
}

pub fn management_routes() -> Router<ApiState> {
    Router::new()
        .route("/servers", get(super::handlers::list_servers))
        .route(
            "/servers/{name}/status",
            get(super::handlers::server_status),
        )
        .route("/servers/{name}/start", post(super::handlers::start_server))
        .route("/servers/{name}/stop", post(super::handlers::stop_server))
        .route(
            "/servers/{name}/restart",
            post(super::handlers::restart_server),
        )
}

pub fn mcp_routes() -> Router<ApiState> {
    Router::new()
        // Note: /mcp/{path} is handled by nest_service in api/mod.rs for SSE support
        // These REST API endpoints remain for backward compatibility
        .route("/mcp/{path}/tools", get(super::handlers::mcp_list_tools))
        .route(
            "/mcp/{path}/tools/call",
            post(super::handlers::mcp_call_tool),
        )
}
