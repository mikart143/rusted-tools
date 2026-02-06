use crate::endpoint::registry::EndpointType;
use crate::error::Result;
use crate::mcp::McpClient;
use async_trait::async_trait;
use axum::Router;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Trait for unified handling of local and remote MCP endpoint instances.
/// Provides polymorphic interface for endpoint lifecycle management and client access.
#[async_trait]
pub trait EndpointInstance: Send + Sync {
    /// Get the endpoint name
    fn name(&self) -> &str;

    /// Get the URL path for this endpoint
    fn path(&self) -> &str;

    /// Get the endpoint type (Local or Remote)
    fn endpoint_type(&self) -> EndpointType;

    /// Start the endpoint (create and validate client connection)
    async fn start(&mut self) -> Result<()>;

    /// Stop the endpoint (clear cached client)
    async fn stop(&mut self) -> Result<()>;

    /// Get or create the MCP client for this endpoint
    /// Returns cached client if available, creates new one if needed
    async fn get_or_create_client(&self) -> Result<Arc<McpClient>>;

    /// Check if the endpoint is started (has active client)
    fn is_started(&self) -> bool;

    /// Attach HTTP routes for this endpoint to the given router
    /// Different endpoint types implement different routing strategies:
    /// - Local: SSE bridge for stdio → HTTP/SSE translation
    /// - Remote: Direct HTTP reverse proxy
    async fn attach_http_route<S>(
        &self,
        router: Router<S>,
        path: &str,
        ct: CancellationToken,
    ) -> Result<Router<S>>
    where
        S: Clone + Send + Sync + 'static;
}
