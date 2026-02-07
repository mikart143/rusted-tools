use crate::endpoint::EndpointManager;
use crate::error::ProxyError;
use crate::routing::{PathRouter, tool_filter};
use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// Application state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub manager: Arc<EndpointManager>,
    pub router: Arc<PathRouter>,
    pub mcp_request_timeout: Duration,
}

pub(crate) async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "rusted-tools",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

pub(crate) async fn server_info() -> impl IntoResponse {
    Json(json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "authors": env!("CARGO_PKG_AUTHORS"),
    }))
}

pub(crate) async fn list_servers(State(state): State<ApiState>) -> impl IntoResponse {
    let endpoints = state.manager.list_endpoints();
    let endpoint_list: Vec<Value> = endpoints
        .into_iter()
        .map(|info| {
            json!({
                "name": info.name,
                "path": info.path,
                "type": info.endpoint_type.to_string(),
                "status": info.status.to_string(),
            })
        })
        .collect();

    Json(json!({
        "servers": endpoint_list
    }))
}

pub(crate) async fn server_status(
    State(state): State<ApiState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ProxyError> {
    let info = state.manager.get_endpoint_info(&name)?;
    Ok(Json(json!({
        "name": info.name,
        "path": info.path,
        "type": info.endpoint_type.to_string(),
        "status": info.status.to_string(),
    })))
}

pub(crate) async fn start_server(
    State(state): State<ApiState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ProxyError> {
    info!("Received request to start endpoint: {}", name);

    state.manager.start_endpoint(&name).await?;
    Ok(Json(json!({
        "name": name,
        "action": "start",
        "status": "success"
    })))
}

pub(crate) async fn stop_server(
    State(state): State<ApiState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ProxyError> {
    info!("Received request to stop endpoint: {}", name);

    state.manager.stop_endpoint(&name).await?;
    Ok(Json(json!({
        "name": name,
        "action": "stop",
        "status": "success"
    })))
}

pub(crate) async fn restart_server(
    State(state): State<ApiState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ProxyError> {
    info!("Received request to restart endpoint: {}", name);

    state.manager.restart_endpoint(&name).await?;
    Ok(Json(json!({
        "name": name,
        "action": "restart",
        "status": "success"
    })))
}

// MCP-specific handlers

pub(crate) async fn mcp_list_tools(
    State(state): State<ApiState>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, ProxyError> {
    let (client, filter) = state.router.get_client(&path).await?;

    // Call list_tools on the actual MCP client
    let tools = tokio::time::timeout(state.mcp_request_timeout, client.list_tools())
        .await
        .map_err(|_| ProxyError::mcp_timeout(state.mcp_request_timeout))??;

    // Apply filter using the centralized function
    let filtered_tools = tool_filter::apply_tool_filter(tools, filter.as_ref());

    Ok(Json(json!({
        "server": client.server_name(),
        "tools": filtered_tools,
        "filter_active": filter.is_some()
    })))
}

pub(crate) async fn mcp_call_tool(
    State(state): State<ApiState>,
    Path(path): Path<String>,
    Json(payload): Json<Value>,
) -> Result<impl IntoResponse, ProxyError> {
    let (client, filter) = state.router.get_client(&path).await?;

    // Parse the tool call request
    let request: crate::mcp::ToolCallRequest =
        serde_json::from_value(payload).map_err(ProxyError::invalid_request)?;

    // Check if tool is allowed using the centralized function
    if !tool_filter::is_tool_allowed(&request.name, filter.as_ref()) {
        return Err(ProxyError::ToolNotAllowed(request.name));
    }

    // Call the tool
    let response = tokio::time::timeout(state.mcp_request_timeout, client.call_tool(request))
        .await
        .map_err(|_| ProxyError::mcp_timeout(state.mcp_request_timeout))??;
    Ok(Json(json!(response)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use serde_json::Value;

    async fn create_test_state() -> ApiState {
        // Use a simple inline config for unit tests
        use crate::config::{EndpointConfig, EndpointKindConfig};
        use std::collections::HashMap;
        use std::time::Duration;

        let manager = Arc::new(EndpointManager::new());

        let configs = vec![
            EndpointConfig {
                name: "test-local".to_string(),
                endpoint_type: EndpointKindConfig::Local {
                    command: "echo".to_string(),
                    args: vec!["hello".to_string()],
                    env: HashMap::new(),
                    auto_start: true,
                },
                tools: None,
            },
            EndpointConfig {
                name: "test-remote".to_string(),
                endpoint_type: EndpointKindConfig::Remote {
                    url: "http://localhost:8080".to_string(),
                },
                tools: None,
            },
        ];

        manager.init_from_config(configs.clone()).await.unwrap();

        let router = Arc::new(PathRouter::new(manager.clone()));

        ApiState {
            manager,
            router,
            mcp_request_timeout: Duration::from_secs(30),
        }
    }

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["status"], "ok");
        assert_eq!(json["service"], "rusted-tools");
        assert!(json["version"].is_string());
    }

    #[tokio::test]
    async fn test_server_info() {
        let response = server_info().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["name"], "rusted-tools");
        assert!(json["version"].is_string());
        assert!(json["description"].is_string());
    }

    #[tokio::test]
    async fn test_list_servers() {
        let state = create_test_state().await;
        let response = list_servers(State(state)).await.into_response();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        let servers = json["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 2);

        // Check local server
        let local = servers.iter().find(|s| s["name"] == "test-local").unwrap();
        assert_eq!(local["type"], "local");
        assert_eq!(local["path"], "test-local");

        // Check remote server
        let remote = servers.iter().find(|s| s["name"] == "test-remote").unwrap();
        assert_eq!(remote["type"], "remote");
        assert_eq!(remote["path"], "test-remote");
    }

    #[tokio::test]
    async fn test_server_status_found() {
        let state = create_test_state().await;
        let response = server_status(State(state), Path("test-local".to_string()))
            .await
            .unwrap()
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["name"], "test-local");
        assert_eq!(json["type"], "local");
        assert!(json["status"].is_string());
    }

    #[tokio::test]
    async fn test_server_status_not_found() {
        let state = create_test_state().await;
        let result = server_status(State(state), Path("nonexistent".to_string())).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_server_not_found() {
        let state = create_test_state().await;
        let result = start_server(State(state), Path("nonexistent".to_string())).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stop_server_not_found() {
        let state = create_test_state().await;
        let result = stop_server(State(state), Path("nonexistent".to_string())).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_restart_server_not_found() {
        let state = create_test_state().await;
        let result = restart_server(State(state), Path("nonexistent".to_string())).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mcp_list_tools_server_not_found() {
        let state = create_test_state().await;
        let result = mcp_list_tools(State(state), Path("nonexistent".to_string())).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mcp_call_tool_server_not_found() {
        let state = create_test_state().await;
        let payload = json!({
            "name": "test_tool",
            "arguments": {}
        });
        let result =
            mcp_call_tool(State(state), Path("nonexistent".to_string()), Json(payload)).await;

        assert!(result.is_err());
    }
}
