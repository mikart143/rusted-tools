use crate::mcp::McpClient;
use std::sync::Arc;

/// Shared MCP client lifecycle helper.
/// Encapsulates a single shared `McpClient` instance
/// used by both LocalEndpoint and RemoteEndpoint.
#[derive(Clone)]
pub(crate) struct ClientHolder {
    client: Arc<McpClient>,
}

impl ClientHolder {
    pub(crate) fn new(name: String) -> Self {
        Self {
            client: Arc::new(McpClient::new(name)),
        }
    }

    pub(crate) fn get(&self) -> Arc<McpClient> {
        self.client.clone()
    }
}
