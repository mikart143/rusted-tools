// MCP SSE Service factory for creating HTTP/SSE endpoints for local MCP endpoints

use crate::mcp::StdioBridge;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Create a StreamableHttpService for a local MCP endpoint
/// This service will forward all MCP protocol messages to the stdio-based local MCP client
///
/// NOTE: This is only for local endpoints. Remote endpoints use axum-reverse-proxy instead.
/// See api/mod.rs for the routing logic.
pub fn create_local_sse_service(
    client: Arc<crate::mcp::McpClient>,
    server_name: String,
    cancellation_token: CancellationToken,
) -> StreamableHttpService<StdioBridge, LocalSessionManager> {
    let client_clone = client.clone();
    let server_name_clone = server_name.clone();

    // Create a factory function that creates a new bridge server instance
    // This will be called for each new SSE session
    // The factory must be sync, so we clone the already-initialized client
    let service_factory = move || {
        Ok(StdioBridge::new(
            client_clone.clone(),
            server_name_clone.clone(),
        ))
    };

    // Create the SSE service with default config
    StreamableHttpService::new(
        service_factory,
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig {
            stateful_mode: true,
            sse_keep_alive: Some(std::time::Duration::from_secs(15)),
            sse_retry: Some(std::time::Duration::from_secs(3)),
            cancellation_token,
        },
    )
}
