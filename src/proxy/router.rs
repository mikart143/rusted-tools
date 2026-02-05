use crate::config::{McpServerConfig, ToolFilter};
use crate::error::{ProxyError, Result};
use crate::proxy::client::McpClient;
use crate::server::ServerManager;
use dashmap::DashMap;
use std::sync::Arc;

/// Router that maps paths to MCP server instances
#[derive(Clone)]
pub struct Router {
    manager: Arc<ServerManager>,
    path_to_server: Arc<DashMap<String, ServerRoute>>,
}

/// Information about a server route
#[derive(Clone)]
struct ServerRoute {
    server_name: String,
    tool_filter: Option<ToolFilter>,
}

impl Router {
    pub fn new(manager: Arc<ServerManager>) -> Self {
        Self {
            manager,
            path_to_server: Arc::new(DashMap::new()),
        }
    }

    /// Initialize routes from configuration
    pub fn init_from_config(&self, configs: &[McpServerConfig]) -> Result<()> {
        for config in configs {
            let path = config.get_path();
            let route = ServerRoute {
                server_name: config.name.clone(),
                tool_filter: config.tools.clone(),
            };

            self.path_to_server.insert(path, route);
        }

        Ok(())
    }

    /// Get server name and filter for a path
    pub fn get_route(&self, path: &str) -> Result<(String, Option<ToolFilter>)> {
        self.path_to_server
            .get(path)
            .map(|entry| {
                let route = entry.value();
                (route.server_name.clone(), route.tool_filter.clone())
            })
            .ok_or_else(|| ProxyError::ServerNotFound(format!("No server at path: {}", path)))
    }

    /// Get MCP client for a specific path
    pub async fn get_client(&self, path: &str) -> Result<(McpClient, Option<ToolFilter>)> {
        let (server_name, tool_filter) = self.get_route(path)?;

        // Get server info to determine type
        let server_info = self.manager.get_server_info(&server_name)?;

        // Get the actual MCP client based on server type
        let client = match server_info.server_type {
            crate::server::ServerType::Local => {
                // Get the local server and its client
                let local_server = self.manager.get_local_server(&server_name)?;
                let server_lock = local_server.read().await;
                server_lock.get_client().await?
            }
            crate::server::ServerType::Remote => {
                // For remote servers, we need to create a fresh client for REST API calls
                // since the cached clients in ServerManager are for SSE bridge only.
                // This allows REST API to work even if SSE initialization failed.
                let remote_server = self.manager.get_remote_server(&server_name)?;
                let client = McpClient::new(server_name.clone());
                
                // Initialize the client with the remote server's URL
                client.init_with_http(&remote_server.url).await?;
                
                client
            }
        };

        Ok((client, tool_filter))
    }

    /// List all routes
    pub fn list_routes(&self) -> Vec<(String, String)> {
        self.path_to_server
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().server_name.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpServerType;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_router_init_and_get_route() {
        let manager = Arc::new(ServerManager::new());

        let config = McpServerConfig {
            name: "test-server".to_string(),
            server_type: McpServerType::Local {
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

        manager.init_from_config(vec![config.clone()]).await.unwrap();

        let router = Router::new(manager);
        router.init_from_config(&[config]).unwrap();

        let (server_name, filter) = router.get_route("test-path").unwrap();
        assert_eq!(server_name, "test-server");
        assert!(filter.is_some());
    }

    #[tokio::test]
    async fn test_router_get_client_remote_unreachable() {
        // Test that router handles unreachable remote servers appropriately
        let manager = Arc::new(ServerManager::new());

        let config = McpServerConfig {
            name: "test-server".to_string(),
            server_type: McpServerType::Remote {
                url: "http://localhost:8080".to_string(),
            },
            tools: None,
            path: Some("remote".to_string()),
        };

        // init_from_config will try to initialize the remote client for SSE bridge
        // Since localhost:8080 doesn't exist, it will skip with a warning (not fail)
        manager.init_from_config(vec![config.clone()]).await.unwrap();

        let router = Router::new(manager);
        router.init_from_config(&[config]).unwrap();

        // get_client creates a fresh client for REST API, which will fail since server is unreachable
        let result = router.get_client("remote").await;
        assert!(result.is_err(), "Should fail when remote server is unreachable");
    }
}
