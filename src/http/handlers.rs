use crate::error::ProxyError;
use crate::proxy::{filter, Router};
use crate::server::ServerManager;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<ServerManager>,
    pub router: Arc<Router>,
}

pub async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "rusted-tools",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

pub async fn server_info() -> impl IntoResponse {
    Json(json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "authors": env!("CARGO_PKG_AUTHORS"),
    }))
}

pub async fn list_servers(State(state): State<AppState>) -> impl IntoResponse {
    let servers = state.manager.list_servers();
    let server_list: Vec<Value> = servers
        .into_iter()
        .map(|info| {
            json!({
                "name": info.name,
                "path": info.path,
                "type": info.server_type.to_string(),
                "status": info.status.to_string(),
            })
        })
        .collect();

    Json(json!({
        "servers": server_list
    }))
}

pub async fn server_status(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ProxyError> {
    let info = state.manager.get_server_info(&name)?;
    Ok(Json(json!({
        "name": info.name,
        "path": info.path,
        "type": info.server_type.to_string(),
        "status": info.status.to_string(),
    })))
}

pub async fn start_server(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ProxyError> {
    info!("Received request to start server: {}", name);

    state.manager.start_server(&name).await?;
    Ok(Json(json!({
        "name": name,
        "action": "start",
        "status": "success"
    })))
}

pub async fn stop_server(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ProxyError> {
    info!("Received request to stop server: {}", name);

    state.manager.stop_server(&name).await?;
    Ok(Json(json!({
        "name": name,
        "action": "stop",
        "status": "success"
    })))
}

pub async fn restart_server(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ProxyError> {
    info!("Received request to restart server: {}", name);

    state.manager.restart_server(&name).await?;
    Ok(Json(json!({
        "name": name,
        "action": "restart",
        "status": "success"
    })))
}

// MCP-specific handlers

pub async fn mcp_sse(Path(path): Path<String>) -> impl IntoResponse {
    // SSE/HTTP transport for MCP protocol is not yet implemented
    // 
    // The proxy currently supports REST-style tool calling via:
    // - GET /mcp/{path}/tools - List available tools
    // - POST /mcp/{path}/tools/call - Call a specific tool
    //
    // For native MCP protocol support via SSE, use one of these approaches:
    // 1. Connect directly to MCP servers (not through proxy)
    // 2. Use the REST API endpoints instead
    // 3. Wait for SSE implementation in a future release
    //
    // See: https://github.com/YOUR_REPO for documentation
    
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "SSE transport not implemented",
            "message": "The MCP HTTP/SSE transport is not yet implemented. Use REST endpoints instead.",
            "available_endpoints": {
                "list_tools": format!("GET /mcp/{}/tools", path),
                "call_tool": format!("POST /mcp/{}/tools/call", path)
            },
            "documentation": "See README for REST API usage"
        }))
    )
}

pub async fn mcp_messages(Path(_path): Path<String>) -> impl IntoResponse {
    // MCP message handling will be added in a future enhancement
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "MCP message endpoint not yet implemented"
        })),
    )
}

pub async fn mcp_list_tools(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, ProxyError> {
    let (client, filter) = state.router.get_client(&path).await?;
    
    // Call list_tools on the actual MCP client
    let tools = client.list_tools().await?;
    
    // Apply filter using the centralized function
    let filtered_tools = filter::apply_tool_filter(tools, filter.as_ref());

    Ok(Json(json!({
        "server": client.server_name(),
        "tools": filtered_tools,
        "filter_active": filter.is_some()
    })))
}

pub async fn mcp_call_tool(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Json(payload): Json<Value>,
) -> Result<impl IntoResponse, ProxyError> {
    let (client, filter) = state.router.get_client(&path).await?;
    
    // Parse the tool call request
    let request: crate::proxy::client::ToolCallRequest =
        serde_json::from_value(payload).map_err(|e| {
            ProxyError::InvalidRequest(format!("Invalid request format: {}", e))
        })?;

    // Check if tool is allowed using the centralized function
    if !filter::is_tool_allowed(&request.name, filter.as_ref()) {
        return Err(ProxyError::ToolNotAllowed(request.name));
    }

    // Call the tool
    let response = client.call_tool(request).await?;
    Ok(Json(json!(response)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    async fn create_test_state() -> AppState {
        // Use a simple inline config for unit tests
        use crate::config::{McpServerConfig, McpServerType};
        use std::collections::HashMap;

        let manager = Arc::new(ServerManager::new());
        
        let configs = vec![
            McpServerConfig {
                name: "test-local".to_string(),
                server_type: McpServerType::Local {
                    command: "echo".to_string(),
                    args: vec!["hello".to_string()],
                    env: HashMap::new(),
                    auto_start: true,
                    restart_on_failure: false,
                },
                tools: None,
                path: Some("test-local".to_string()),
            },
            McpServerConfig {
                name: "test-remote".to_string(),
                server_type: McpServerType::Remote {
                    url: "http://localhost:8080".to_string(),
                },
                tools: None,
                path: Some("test-remote".to_string()),
            },
        ];
        
        manager.init_from_config(configs.clone()).await.unwrap();

        let router = Arc::new(Router::new(manager.clone()));
        router.init_from_config(&configs).unwrap();
        
        AppState { manager, router }
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
    async fn test_mcp_sse_not_implemented() {
        let response = mcp_sse(Path("test".to_string())).await.into_response();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[tokio::test]
    async fn test_mcp_messages_not_implemented() {
        let response = mcp_messages(Path("test".to_string())).await.into_response();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
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
        let result = mcp_call_tool(
            State(state),
            Path("nonexistent".to_string()),
            Json(payload)
        ).await;
        
        assert!(result.is_err());
    }
}

