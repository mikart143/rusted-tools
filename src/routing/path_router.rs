use crate::config::ToolFilter;
use crate::endpoint::EndpointManager;
use crate::error::Result;
use crate::mcp::McpClient;
use std::sync::Arc;

/// Router that maps paths to MCP endpoint instances
#[derive(Clone)]
pub struct PathRouter {
    manager: Arc<EndpointManager>,
}

impl PathRouter {
    pub fn new(manager: Arc<EndpointManager>) -> Self {
        Self { manager }
    }

    /// Get endpoint name and filter for a path
    pub(crate) fn get_route(&self, path: &str) -> Result<(String, Option<ToolFilter>)> {
        let info = self.manager.get_endpoint_info_by_path(path)?;
        Ok((info.name, info.tool_filter))
    }

    /// Get MCP client for a specific path (works for both local and remote)
    pub(crate) async fn get_client(
        &self,
        path: &str,
    ) -> Result<(Arc<McpClient>, Option<ToolFilter>)> {
        let (endpoint_name, tool_filter) = self.get_route(path)?;

        // Get client using polymorphic manager method
        let client = self.manager.get_client(&endpoint_name).await?;

        Ok((client, tool_filter))
    }

    /// List all routes
    pub(crate) fn list_routes(&self) -> Vec<(String, String)> {
        self.manager
            .list_endpoints()
            .into_iter()
            .map(|info| (info.path, info.name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EndpointConfig, EndpointKindConfig};
    use crate::error::ProxyError;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_router_init_and_get_route() {
        let manager = Arc::new(EndpointManager::new());

        let config = EndpointConfig {
            name: "test-server".to_string(),
            endpoint_type: EndpointKindConfig::Local {
                command: "echo".to_string(),
                args: vec![],
                env: HashMap::new(),
                auto_start: false,
            },
            tools: Some(ToolFilter {
                include: Some(vec!["tool1".to_string()]),
                exclude: None,
            }),
        };

        manager
            .init_from_config(vec![config.clone()])
            .await
            .unwrap();

        let router = PathRouter::new(manager);

        let (endpoint_name, filter) = router.get_route("test-server").unwrap();
        assert_eq!(endpoint_name, "test-server");
        assert!(filter.is_some());
    }

    #[tokio::test]
    async fn test_router_get_client_remote_unreachable() {
        // Test that router handles unreachable remote endpoints appropriately
        let manager = Arc::new(EndpointManager::new());

        let config = EndpointConfig {
            name: "test-server".to_string(),
            endpoint_type: EndpointKindConfig::Remote {
                url: "http://localhost:8080".to_string(),
            },
            tools: None,
        };

        manager
            .init_from_config(vec![config.clone()])
            .await
            .unwrap();

        let router = PathRouter::new(manager);

        let result = router.get_client("test-server").await;
        assert!(
            matches!(result, Err(ProxyError::ServerNotRunning(_))),
            "Should require explicit start before creating a client"
        );
    }
}
