use super::runtime::{McpRuntimeHandle, RuntimeState, spawn_runtime};
use super::types::{ToolCallRequest, ToolCallResponse, ToolDefinition};
use crate::error::{ProxyError, Result};
use rmcp::ServiceExt;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Default timeout for MCP handshake initialization.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

/// Type alias for the runtime handle stored in RwLock
type RuntimeHandleType = Arc<RwLock<Option<McpRuntimeHandle>>>;

/// A wrapper around rmcp RunningService for the proxy
#[derive(Clone)]
pub(crate) struct McpClient {
    server_name: String,
    runtime: RuntimeHandleType,
}

impl McpClient {
    pub(crate) fn new(server_name: String) -> Self {
        Self {
            server_name,
            runtime: Arc::new(RwLock::new(None)),
        }
    }

    async fn ensure_not_running(&self) -> Result<()> {
        let mut runtime_lock = self.runtime.write().await;
        if let Some(runtime) = runtime_lock.as_ref() {
            match runtime.state().await {
                RuntimeState::Running => {
                    return Err(ProxyError::server_already_running(self.server_name.clone()));
                }
                RuntimeState::Stopped | RuntimeState::Failed(_) => {
                    *runtime_lock = None;
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn is_running(&self) -> bool {
        if let Some(runtime) = self.runtime.read().await.as_ref() {
            matches!(runtime.state().await, RuntimeState::Running)
        } else {
            false
        }
    }

    /// Initialize the MCP client with TokioChildProcess transport
    pub(crate) async fn init_with_transport(&self, transport: TokioChildProcess) -> Result<()> {
        self.ensure_not_running().await?;
        info!("Initializing MCP client for server: {}", self.server_name);

        let ct = CancellationToken::new();
        let ct_clone = ct.clone();

        let service = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            ().serve_with_ct(transport, ct_clone).await
        })
        .await
        .map_err(|_| {
            ct.cancel();
            ProxyError::mcp_handshake_timeout(HANDSHAKE_TIMEOUT, &self.server_name, None)
        })?
        .map_err(|e| {
            ProxyError::mcp_protocol(format!("Failed to initialize MCP client: {:?}", e))
        })?;

        let runtime = spawn_runtime(self.server_name.clone(), service);
        let mut runtime_lock = self.runtime.write().await;
        *runtime_lock = Some(runtime);

        debug!("MCP client initialized for server: {}", self.server_name);
        Ok(())
    }

    /// Initialize the MCP client with HTTP transport for remote servers
    pub(crate) async fn init_with_http(&self, url: &str) -> Result<()> {
        self.ensure_not_running().await?;
        info!(
            "Initializing MCP HTTP client for server: {} at {}",
            self.server_name, url
        );

        let transport = StreamableHttpClientTransport::from_uri(url);

        let ct = CancellationToken::new();
        let ct_clone = ct.clone();

        let service = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            ().serve_with_ct(transport, ct_clone).await
        })
        .await
        .map_err(|_| {
            ct.cancel();
            ProxyError::mcp_handshake_timeout(HANDSHAKE_TIMEOUT, &self.server_name, Some(url))
        })?
        .map_err(|e| {
            ProxyError::mcp_protocol(format!("Failed to initialize MCP HTTP client: {:?}", e))
        })?;

        let runtime = spawn_runtime(self.server_name.clone(), service);
        let mut runtime_lock = self.runtime.write().await;
        *runtime_lock = Some(runtime);

        debug!(
            "MCP HTTP client initialized for server: {}",
            self.server_name
        );
        Ok(())
    }

    /// List available tools from the MCP server
    pub(crate) async fn list_tools(&self) -> Result<Vec<ToolDefinition>> {
        let runtime = self
            .runtime
            .read()
            .await
            .as_ref()
            .cloned()
            .ok_or_else(|| ProxyError::server_not_running(self.server_name.clone()))?;

        runtime.list_tools(&self.server_name).await
    }

    /// Call a tool on the MCP server
    pub(crate) async fn call_tool(&self, request: ToolCallRequest) -> Result<ToolCallResponse> {
        let runtime = self
            .runtime
            .read()
            .await
            .as_ref()
            .cloned()
            .ok_or_else(|| ProxyError::server_not_running(self.server_name.clone()))?;

        runtime.call_tool(&self.server_name, request).await
    }

    /// Get server name
    pub(crate) fn server_name(&self) -> &str {
        &self.server_name
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        let runtime = {
            let mut runtime_lock = self.runtime.write().await;
            runtime_lock
                .take()
                .ok_or_else(|| ProxyError::server_not_running(self.server_name.clone()))?
        };

        runtime.stop(&self.server_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_client() {
        let client = McpClient::new("test-server".to_string());
        assert_eq!(client.server_name(), "test-server");
    }

    #[tokio::test]
    async fn test_client_not_initialized() {
        let client = McpClient::new("test-server".to_string());

        // Attempting to use an uninitialized client should fail
        let result = client.list_tools().await;
        assert!(result.is_err());

        // Error should indicate server is not running
        if let Err(e) = result {
            assert!(e.to_string().contains("not running"));
        }
    }
}
