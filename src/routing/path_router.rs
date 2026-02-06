use crate::config::{EndpointConfig, ToolFilter};
use crate::endpoint::EndpointManager;
use crate::error::{ProxyError, Result};
use crate::mcp::McpClient;
use dashmap::DashMap;
use std::sync::Arc;

/// Router that maps paths to MCP endpoint instances
#[derive(Clone)]
pub struct PathRouter {
    manager: Arc<EndpointManager>,
    path_to_endpoint: Arc<DashMap<String, EndpointRoute>>,
}

/// Information about an endpoint route
#[derive(Clone)]
struct EndpointRoute {
    endpoint_name: String,
    tool_filter: Option<ToolFilter>,
}

impl PathRouter {
    pub fn new(manager: Arc<EndpointManager>) -> Self {
        Self {
            manager,
            path_to_endpoint: Arc::new(DashMap::new()),
        }
    }

    /// Initialize routes from configuration
    pub fn init_from_config(&self, configs: &[EndpointConfig]) -> Result<()> {
        for config in configs {
            let path = config.get_path();
            let route = EndpointRoute {
                endpoint_name: config.name.clone(),
                tool_filter: config.tools.clone(),
            };

            self.path_to_endpoint.insert(path, route);
        }

        Ok(())
    }

    /// Get endpoint name and filter for a path
    pub fn get_route(&self, path: &str) -> Result<(String, Option<ToolFilter>)> {
        self.path_to_endpoint
            .get(path)
            .map(|entry| {
                let route = entry.value();
                (route.endpoint_name.clone(), route.tool_filter.clone())
            })
            .ok_or_else(|| ProxyError::ServerNotFound(format!("No endpoint at path: {}", path)))
    }

    /// Get MCP client for a specific path (works for both local and remote)
    pub async fn get_client(&self, path: &str) -> Result<(Arc<McpClient>, Option<ToolFilter>)> {
        let (endpoint_name, tool_filter) = self.get_route(path)?;

        // Get client using polymorphic manager method
        let client = self.manager.get_client(&endpoint_name).await?;

        Ok((client, tool_filter))
    }

    /// List all routes
    pub fn list_routes(&self) -> Vec<(String, String)> {
        self.path_to_endpoint
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().endpoint_name.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EndpointKindConfig;
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
                restart_on_failure: false,
            },
            tools: Some(ToolFilter {
                include: Some(vec!["tool1".to_string()]),
                exclude: None,
            }),
            path: Some("test-path".to_string()),
        };

        manager
            .init_from_config(vec![config.clone()])
            .await
            .unwrap();

        let router = PathRouter::new(manager);
        router.init_from_config(&[config]).unwrap();

        let (endpoint_name, filter) = router.get_route("test-path").unwrap();
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
            path: Some("remote".to_string()),
        };

        manager
            .init_from_config(vec![config.clone()])
            .await
            .unwrap();

        let router = PathRouter::new(manager);
        router.init_from_config(&[config]).unwrap();

        // get_client creates a client, which will fail since endpoint is unreachable
        let result = router.get_client("remote").await;
        assert!(
            result.is_err(),
            "Should fail when remote endpoint is unreachable"
        );
    }
}
