use crate::config::{McpServerConfig, McpServerType};
use crate::error::{ProxyError, Result};
use crate::server::local::{extract_local_config, LocalMcpServer};
use crate::server::registry::{ServerInfo, ServerRegistry, ServerStatus, ServerType};
use crate::server::remote::RemoteMcpServer;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Manager for all MCP server instances (local and remote)
#[derive(Clone)]
pub struct ServerManager {
    registry: ServerRegistry,
    local_servers: Arc<DashMap<String, Arc<RwLock<LocalMcpServer>>>>,
    remote_servers: Arc<DashMap<String, RemoteMcpServer>>,
}

impl ServerManager {
    pub fn new() -> Self {
        Self {
            registry: ServerRegistry::new(),
            local_servers: Arc::new(DashMap::new()),
            remote_servers: Arc::new(DashMap::new()),
        }
    }

    /// Initialize servers from configuration
    pub async fn init_from_config(&self, configs: Vec<McpServerConfig>) -> Result<()> {
        info!("Initializing {} MCP servers from configuration", configs.len());

        for config in configs {
            let path = config.get_path();
            let name = config.name.clone();

            match &config.server_type {
                McpServerType::Local { auto_start, .. } => {
                    // Register local server
                    self.registry
                        .register(name.clone(), path.clone(), ServerType::Local)?;

                    // Create local server instance
                    let local_config = extract_local_config(&config)?;
                    let server = LocalMcpServer::new(name.clone(), local_config);
                    self.local_servers
                        .insert(name.clone(), Arc::new(RwLock::new(server)));

                    // Auto-start if configured
                    if *auto_start {
                        info!("Auto-starting local server: {}", name);
                        if let Err(e) = self.start_server(&name).await {
                            error!("Failed to auto-start server {}: {}", name, e);
                        }
                    }
                }
                McpServerType::Remote { .. } => {
                    // Register remote server
                    self.registry
                        .register(name.clone(), path.clone(), ServerType::Remote)?;
                    
                    // Create remote server instance
                    let remote_server = RemoteMcpServer::from_config(&config)?;
                    self.remote_servers.insert(name.clone(), remote_server.clone());
                    
                    // Remote servers use direct HTTP reverse proxy (no client initialization needed)
                    info!("Registered remote server: {} at path /{} → {}", name, path, remote_server.url);
                }
            }
        }

        Ok(())
    }

    /// Start a local MCP server
    pub async fn start_server(&self, name: &str) -> Result<()> {
        // Check if server exists in registry
        let info = self.registry.get(name)?;

        // Only local servers can be started
        if !matches!(info.server_type, ServerType::Local) {
            return Err(ProxyError::InvalidRequest(format!(
                "Server {} is not a local server",
                name
            )));
        }

        // Check current status
        if info.status == ServerStatus::Running {
            return Err(ProxyError::ServerAlreadyRunning(name.to_string()));
        }

        // Update status to starting
        self.registry.set_status(name, ServerStatus::Starting)?;

        // Get the local server instance
        let server_lock = self
            .local_servers
            .get(name)
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))?;

        let mut server = server_lock.write().await;

        // Start the server
        match server.start().await {
            Ok(()) => {
                self.registry.set_status(name, ServerStatus::Running)?;
                info!("Successfully started server: {}", name);
                Ok(())
            }
            Err(e) => {
                self.registry.set_status(name, ServerStatus::Failed)?;
                error!("Failed to start server {}: {}", name, e);
                Err(e)
            }
        }
    }

    /// Stop a local MCP server
    pub async fn stop_server(&self, name: &str) -> Result<()> {
        // Check if server exists in registry
        let info = self.registry.get(name)?;

        // Only local servers can be stopped
        if !matches!(info.server_type, ServerType::Local) {
            return Err(ProxyError::InvalidRequest(format!(
                "Server {} is not a local server",
                name
            )));
        }

        // Check current status - return error if already stopped
        if info.status == ServerStatus::Stopped {
            return Err(ProxyError::ServerNotRunning(name.to_string()));
        }

        // Update status to stopping
        self.registry.set_status(name, ServerStatus::Stopping)?;

        // Get the local server instance
        let server_lock = self
            .local_servers
            .get(name)
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))?;

        let mut server = server_lock.write().await;

        // Stop the server
        match server.stop().await {
            Ok(()) => {
                self.registry.set_status(name, ServerStatus::Stopped)?;
                info!("Successfully stopped server: {}", name);
                Ok(())
            }
            Err(e) => {
                error!("Failed to stop server {}: {}", name, e);
                Err(e)
            }
        }
    }

    /// Restart a local MCP server
    pub async fn restart_server(&self, name: &str) -> Result<()> {
        info!("Restarting server: {}", name);
        self.stop_server(name).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start_server(name).await?;
        Ok(())
    }

    /// Get server info by name
    pub fn get_server_info(&self, name: &str) -> Result<ServerInfo> {
        self.registry.get(name)
    }

    /// List all registered servers
    pub fn list_servers(&self) -> Vec<ServerInfo> {
        self.registry.list()
    }

    /// Get a reference to a local server
    pub fn get_local_server(&self, name: &str) -> Result<Arc<RwLock<LocalMcpServer>>> {
        self.local_servers
            .get(name)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))
    }

    /// Get a remote server by name
    pub fn get_remote_server(&self, name: &str) -> Result<RemoteMcpServer> {
        self.remote_servers
            .get(name)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))
    }

    /// Get an MCP client for a local server by name (for SSE bridge)
    /// Note: This only works for local servers. Remote servers use direct HTTP reverse proxy.
    pub async fn get_mcp_client(&self, name: &str) -> Result<Arc<crate::proxy::McpClient>> {
        // Get server info to verify it's a local server
        let server_info = self.registry.get(name)?;
        
        if !matches!(server_info.server_type, ServerType::Local) {
            return Err(ProxyError::InvalidRequest(format!(
                "get_mcp_client() only works for local servers. '{}' is a remote server.",
                name
            )));
        }
        
        // Get the local server and its client
        let server = self.get_local_server(name)?;
        let server_guard = server.read().await;
        let client = server_guard.get_client().await?;
        Ok(Arc::new(client))
    }

    /// Shutdown all servers
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down all local servers");

        for entry in self.local_servers.iter() {
            let name = entry.key();
            if let Err(e) = self.stop_server(name).await {
                warn!("Error stopping server {} during shutdown: {}", name, e);
            }
        }

        Ok(())
    }
}

impl Default for ServerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpServerType;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_init_local_server_no_autostart() {
        let manager = ServerManager::new();

        let config = McpServerConfig {
            name: "test-server".to_string(),
            server_type: McpServerType::Local {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                env: HashMap::new(),
                auto_start: false,
                restart_on_failure: false,
            },
            tools: None,
            path: Some("test".to_string()),
        };

        manager.init_from_config(vec![config]).await.unwrap();

        let info = manager.get_server_info("test-server").unwrap();
        assert_eq!(info.status, ServerStatus::Stopped);
    }

    #[tokio::test]
    async fn test_start_and_stop_server() {
        let manager = ServerManager::new();

        let config = McpServerConfig {
            name: "test-cat".to_string(),
            server_type: McpServerType::Local {
                command: "cat".to_string(),
                args: vec![],
                env: HashMap::new(),
                auto_start: false,
                restart_on_failure: false,
            },
            tools: None,
            path: Some("test".to_string()),
        };

        manager.init_from_config(vec![config]).await.unwrap();

        // Start the server
        manager.start_server("test-cat").await.unwrap();
        let info = manager.get_server_info("test-cat").unwrap();
        assert_eq!(info.status, ServerStatus::Running);

        // Stop the server
        manager.stop_server("test-cat").await.unwrap();
        let info = manager.get_server_info("test-cat").unwrap();
        assert_eq!(info.status, ServerStatus::Stopped);
    }

    #[tokio::test]
    async fn test_remote_server_registration() {
        let manager = ServerManager::new();

        let config = McpServerConfig {
            name: "remote-server".to_string(),
            server_type: McpServerType::Remote {
                url: "https://example.com".to_string(),
            },
            tools: None,
            path: Some("remote".to_string()),
        };

        manager.init_from_config(vec![config]).await.unwrap();

        let info = manager.get_server_info("remote-server").unwrap();
        assert!(matches!(info.server_type, ServerType::Remote));

        // Cannot start remote servers
        let result = manager.start_server("remote-server").await;
        assert!(result.is_err());
    }
}
