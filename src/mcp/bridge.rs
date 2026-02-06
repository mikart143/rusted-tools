// Bridge between HTTP/SSE and stdio MCP transports (for local endpoints only)
// This module creates an MCP server that forwards all requests to a stdio-based local MCP client
// For remote HTTP/SSE endpoints, use axum-reverse-proxy instead (see api/mod.rs)

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use std::sync::Arc;
use tracing::{debug, warn};

use super::client::McpClient;

/// MCP Server implementation that bridges stdio-based local MCP to HTTP/SSE
/// This translates HTTP/SSE requests into stdio protocol for local endpoints.
/// Remote endpoints use direct HTTP reverse proxy instead.
#[derive(Clone)]
pub(crate) struct StdioBridge {
    client: Arc<McpClient>,
    server_name: String,
}

impl StdioBridge {
    pub(crate) fn new(client: Arc<McpClient>, server_name: String) -> Self {
        Self {
            client,
            server_name,
        }
    }
}

// Implement the MCP ServerHandler trait
impl ServerHandler for StdioBridge {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(format!("Proxy to {} MCP server", self.server_name)),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }

    // List tools - forward to stdio client
    async fn list_tools(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        debug!("Bridge server listing tools");
        let tools =
            self.client.list_tools().await.map_err(|e| {
                McpError::internal_error(format!("Failed to list tools: {}", e), None)
            })?;

        // Convert our ToolDefinition format to rmcp::model::Tool
        let mcp_tools: Vec<rmcp::model::Tool> = tools
            .into_iter()
            .map(|t| rmcp::model::Tool {
                name: t.name.into(),
                title: None,
                description: t.description.map(Into::into),
                input_schema: Arc::new(t.input_schema.as_object().cloned().unwrap_or_default()),
                output_schema: None,
                annotations: None,
                icons: None,
                meta: None,
            })
            .collect();

        Ok(ListToolsResult {
            meta: None,
            tools: mcp_tools,
            next_cursor: None,
        })
    }

    // Call tool - forward to stdio client
    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        debug!("Bridge server calling tool: {}", params.name);

        let tool_request = super::types::ToolCallRequest {
            name: params.name.to_string(),
            arguments: serde_json::Value::Object(params.arguments.unwrap_or_default()),
        };

        let response =
            self.client.call_tool(tool_request).await.map_err(|e| {
                McpError::internal_error(format!("Failed to call tool: {}", e), None)
            })?;

        // Convert our response to rmcp format
        let content: Vec<rmcp::model::Content> = response
            .content
            .into_iter()
            .map(|c| match c {
                super::types::ToolContent::Text { text } => rmcp::model::Content::text(text),
                super::types::ToolContent::Image { data, mime_type } => {
                    rmcp::model::Content::image(data, mime_type)
                }
                super::types::ToolContent::Resource { uri, mime_type } => {
                    warn!("Resource content type not fully supported yet: {}", uri);
                    rmcp::model::Content::text(format!(
                        "Resource: {} ({})",
                        uri,
                        mime_type.unwrap_or_else(|| "unknown".to_string())
                    ))
                }
            })
            .collect();

        Ok(CallToolResult {
            meta: None,
            content,
            structured_content: None,
            is_error: response.is_error,
        })
    }
}
