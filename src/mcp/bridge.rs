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
use super::types::ToolDefinition;

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
        let tools = self
            .client
            .list_tools()
            .await
            .map_err(|e| e.to_mcp_error("list tools"))?;

        // Convert our ToolDefinition format to rmcp::model::Tool
        let mcp_tools: Vec<rmcp::model::Tool> = tools.into_iter().map(build_rmcp_tool).collect();

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

        let response = self
            .client
            .call_tool(tool_request)
            .await
            .map_err(|e| e.to_mcp_error("call tool"))?;

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

fn build_rmcp_tool(tool: ToolDefinition) -> rmcp::model::Tool {
    let input_schema = match tool.input_schema.as_object() {
        Some(schema) => schema.clone(),
        None => {
            warn!(
                "Tool '{}' has non-object input schema; returning empty object",
                tool.name
            );
            serde_json::Map::new()
        }
    };

    rmcp::model::Tool {
        name: tool.name.into(),
        title: None,
        description: tool.description.map(Into::into),
        input_schema: Arc::new(input_schema),
        output_schema: None,
        annotations: None,
        icons: None,
        meta: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_rmcp_tool_preserves_object_schema() {
        let tool = ToolDefinition {
            name: "example".to_string(),
            description: Some("Example tool".to_string()),
            input_schema: json!({"type": "object"}),
        };

        let converted = build_rmcp_tool(tool);
        assert!(converted.input_schema.contains_key("type"));
    }

    #[test]
    fn test_build_rmcp_tool_non_object_schema_is_empty() {
        let tool = ToolDefinition {
            name: "example".to_string(),
            description: None,
            input_schema: json!(true),
        };

        let converted = build_rmcp_tool(tool);
        assert!(converted.input_schema.is_empty());
    }

    #[test]
    fn test_bridge_list_tools_creates_correct_mcp_tools() {
        let tool = ToolDefinition {
            name: "test_tool".to_string(),
            description: Some("A test tool".to_string()),
            input_schema: json!({"type": "object", "properties": {"arg": {"type": "string"}}}),
        };

        let converted = build_rmcp_tool(tool);
        assert_eq!(converted.name.as_ref(), "test_tool");
        assert_eq!(
            converted.description.as_ref().map(|d| d.as_ref()),
            Some("A test tool")
        );
    }

    #[test]
    fn test_bridge_handles_tool_with_complex_schema() {
        let tool = ToolDefinition {
            name: "complex_tool".to_string(),
            description: Some("Complex tool with nested schema".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "config": {
                        "type": "object",
                        "properties": {
                            "nested": {"type": "string"}
                        }
                    }
                }
            }),
        };

        let converted = build_rmcp_tool(tool);
        assert_eq!(converted.name.as_ref(), "complex_tool");
        assert!(converted.input_schema.contains_key("properties"));
    }

    #[test]
    fn test_bridge_handles_tool_with_null_schema() {
        let tool = ToolDefinition {
            name: "null_tool".to_string(),
            description: Some("Tool with null schema".to_string()),
            input_schema: json!(null),
        };

        let converted = build_rmcp_tool(tool);
        // Non-object schemas become empty object (current behavior)
        assert!(converted.input_schema.is_empty());
    }

    #[test]
    fn test_bridge_handles_tool_with_array_schema() {
        let tool = ToolDefinition {
            name: "array_tool".to_string(),
            description: Some("Tool with array schema".to_string()),
            input_schema: json!([{"type": "string"}]),
        };

        let converted = build_rmcp_tool(tool);
        // Non-object schemas become empty object
        assert!(converted.input_schema.is_empty());
    }

    #[test]
    fn test_build_rmcp_tool_removes_non_object_schema_and_logs_warn() {
        let tool = ToolDefinition {
            name: "string_tool".to_string(),
            description: Some("Tool with string schema".to_string()),
            input_schema: json!("just a string"),
        };

        let converted = build_rmcp_tool(tool);
        assert!(converted.input_schema.is_empty());
        assert_eq!(converted.name.as_ref(), "string_tool");
    }
}
