use super::types::{ToolCallRequest, ToolCallResponse, ToolContent, ToolDefinition};
use crate::error::{ProxyError, Result};
use rmcp::model::{CallToolRequestParams, PaginatedRequestParams, RawContent};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::ServiceExt;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

/// Default timeout for MCP handshake initialization.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

/// A wrapper around rmcp RunningService for the proxy
#[derive(Clone)]
pub(crate) struct McpClient {
    server_name: String,
    service: Arc<RwLock<Option<Arc<RunningService<RoleClient, ()>>>>>,
}

impl McpClient {
    pub(crate) fn new(server_name: String) -> Self {
        Self {
            server_name,
            service: Arc::new(RwLock::new(None)),
        }
    }

    async fn store_service(&self, service: RunningService<RoleClient, ()>) {
        let mut lock = self.service.write().await;
        *lock = Some(Arc::new(service));
    }

    /// Initialize the MCP client with TokioChildProcess transport
    pub(crate) async fn init_with_transport(&self, transport: TokioChildProcess) -> Result<()> {
        info!("Initializing MCP client for server: {}", self.server_name);

        let ct = CancellationToken::new();
        let ct_clone = ct.clone();

        let service = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            ().serve_with_ct(transport, ct_clone).await
        })
        .await
        .map_err(|_| {
            ct.cancel();
            ProxyError::McpProtocol(format!(
                "MCP handshake timed out after {:?} for server: {}",
                HANDSHAKE_TIMEOUT, self.server_name
            ))
        })?
        .map_err(|e| {
            ProxyError::McpProtocol(format!("Failed to initialize MCP client: {:?}", e))
        })?;

        self.store_service(service).await;

        debug!("MCP client initialized for server: {}", self.server_name);
        Ok(())
    }

    /// Initialize the MCP client with HTTP transport for remote servers
    pub(crate) async fn init_with_http(&self, url: &str) -> Result<()> {
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
            ProxyError::McpProtocol(format!(
                "MCP handshake timed out after {:?} for server: {} at {}",
                HANDSHAKE_TIMEOUT, self.server_name, url
            ))
        })?
        .map_err(|e| {
            ProxyError::McpProtocol(format!("Failed to initialize MCP HTTP client: {:?}", e))
        })?;

        self.store_service(service).await;

        debug!(
            "MCP HTTP client initialized for server: {}",
            self.server_name
        );
        Ok(())
    }

    /// List available tools from the MCP server
    pub(crate) async fn list_tools(&self) -> Result<Vec<ToolDefinition>> {
        let service = {
            let service_lock = self.service.read().await;
            service_lock
                .as_ref()
                .cloned()
                .ok_or_else(|| ProxyError::ServerNotRunning(self.server_name.clone()))?
        };

        debug!("Listing tools for server: {}", self.server_name);

        let mut tool_list = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let request = Some(PaginatedRequestParams {
                meta: None,
                cursor: cursor.clone(),
            });

            match service.list_tools(request).await {
                Ok(result) => {
                    tool_list.extend(result.tools.into_iter().map(|t| ToolDefinition {
                        name: t.name.to_string(),
                        description: t.description.map(|d| d.to_string()),
                        input_schema: Value::Object((*t.input_schema).clone()),
                    }));

                    cursor = result.next_cursor;
                    if cursor.is_none() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to list tools for {}: {}", self.server_name, e);
                    return Err(ProxyError::McpProtocol(format!(
                        "Failed to list tools: {}",
                        e
                    )));
                }
            }
        }

        debug!(
            "Found {} tools for server: {}",
            tool_list.len(),
            self.server_name
        );
        Ok(tool_list)
    }

    /// Call a tool on the MCP server
    pub(crate) async fn call_tool(&self, request: ToolCallRequest) -> Result<ToolCallResponse> {
        let service = {
            let service_lock = self.service.read().await;
            service_lock
                .as_ref()
                .cloned()
                .ok_or_else(|| ProxyError::ServerNotRunning(self.server_name.clone()))?
        };

        debug!(
            "Calling tool '{}' on server: {}",
            request.name, self.server_name
        );

        let mcp_request = CallToolRequestParams {
            meta: None,
            name: request.name.clone().into(),
            arguments: request.arguments.as_object().cloned(),
            task: None,
        };

        match service.call_tool(mcp_request).await {
            Ok(result) => {
                let response_content: Vec<ToolContent> = result
                    .content
                    .into_iter()
                    .filter_map(|c| match c.raw {
                        RawContent::Text(text_content) => Some(ToolContent::Text {
                            text: text_content.text,
                        }),
                        RawContent::Image(image_content) => Some(ToolContent::Image {
                            data: image_content.data,
                            mime_type: image_content.mime_type,
                        }),
                        RawContent::Resource(resource_content) => {
                            // Extract URI from ResourceContents
                            match resource_content.resource {
                                rmcp::model::ResourceContents::TextResourceContents {
                                    uri,
                                    mime_type,
                                    ..
                                } => Some(ToolContent::Resource { uri, mime_type }),
                                rmcp::model::ResourceContents::BlobResourceContents {
                                    uri,
                                    mime_type,
                                    ..
                                } => Some(ToolContent::Resource { uri, mime_type }),
                            }
                        }
                        _ => None,
                    })
                    .collect();

                Ok(ToolCallResponse {
                    content: response_content,
                    is_error: result.is_error,
                })
            }
            Err(e) => {
                error!(
                    "Failed to call tool '{}' on {}: {}",
                    request.name, self.server_name, e
                );
                Err(ProxyError::McpProtocol(format!(
                    "Failed to call tool: {}",
                    e
                )))
            }
        }
    }

    /// Get server name
    pub(crate) fn server_name(&self) -> &str {
        &self.server_name
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
