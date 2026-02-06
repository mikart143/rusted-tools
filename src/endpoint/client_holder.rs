use crate::error::{ProxyError, Result};
use crate::mcp::McpClient;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared MCP client lifecycle helper.
/// Encapsulates the `Arc<RwLock<Option<Arc<McpClient>>>>` pattern
/// used by both LocalEndpoint and RemoteEndpoint.
#[derive(Clone)]
pub(crate) struct ClientHolder {
    client: Arc<RwLock<Option<Arc<McpClient>>>>,
}

impl ClientHolder {
    pub(crate) fn new() -> Self {
        Self {
            client: Arc::new(RwLock::new(None)),
        }
    }

    pub(crate) async fn get(&self, name: &str) -> Result<Arc<McpClient>> {
        self.client
            .read()
            .await
            .as_ref()
            .cloned()
            .ok_or_else(|| ProxyError::ServerNotRunning(name.to_string()))
    }

    pub(crate) async fn set(&self, client: McpClient) {
        let mut lock = self.client.write().await;
        *lock = Some(Arc::new(client));
    }

    pub(crate) async fn clear(&self) {
        let mut lock = self.client.write().await;
        *lock = None;
    }

    pub(crate) async fn ensure_not_running(&self, name: &str) -> Result<()> {
        if self.client.read().await.is_some() {
            return Err(ProxyError::ServerAlreadyRunning(name.to_string()));
        }
        Ok(())
    }

    pub(crate) async fn ensure_running(&self, name: &str) -> Result<()> {
        if self.client.read().await.is_none() {
            return Err(ProxyError::ServerNotRunning(name.to_string()));
        }
        Ok(())
    }
}
