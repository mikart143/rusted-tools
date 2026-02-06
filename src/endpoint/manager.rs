use crate::config::{EndpointConfig, EndpointKindConfig};
use crate::endpoint::local::LocalEndpoint;
use crate::endpoint::registry::{EndpointInfo, EndpointRegistry, EndpointStatus, EndpointType};
use crate::endpoint::remote::RemoteEndpoint;
use crate::endpoint::EndpointKind;
use crate::error::{ProxyError, Result};
use crate::mcp::McpClient;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Manager for all MCP endpoint instances (local and remote)
/// Uses polymorphic storage via EndpointKind enum for unified handling
#[derive(Clone)]
pub struct EndpointManager {
    registry: EndpointRegistry,
    endpoints: Arc<DashMap<String, Arc<RwLock<EndpointKind>>>>,
}

impl EndpointManager {
    pub fn new() -> Self {
        Self {
            registry: EndpointRegistry::new(),
            endpoints: Arc::new(DashMap::new()),
        }
    }

    /// Initialize endpoints from configuration
    pub async fn init_from_config(&self, configs: Vec<EndpointConfig>) -> Result<()> {
        info!(
            "Initializing {} MCP endpoints from configuration",
            configs.len()
        );

        for config in configs {
            let endpoint_type = config.endpoint_type.clone();
            match endpoint_type {
                EndpointKindConfig::Local { auto_start, .. } => {
                    self.init_local_endpoint(config, auto_start).await?;
                }
                EndpointKindConfig::Remote { .. } => {
                    self.init_remote_endpoint(config).await?;
                }
            }
        }

        Ok(())
    }

    async fn init_local_endpoint(&self, config: EndpointConfig, auto_start: bool) -> Result<()> {
        let name = config.name.clone();

        self.registry
            .register(name.clone(), name.clone(), EndpointType::Local)?;

        let local_config = config.to_local_settings();
        let endpoint = LocalEndpoint::new(name.clone(), local_config);
        let endpoint_kind = EndpointKind::Local(endpoint);
        self.endpoints
            .insert(name.clone(), Arc::new(RwLock::new(endpoint_kind)));

        if auto_start {
            info!("Auto-starting local endpoint: {}", name);
            if let Err(e) = self.start_endpoint(&name).await {
                error!("Failed to auto-start endpoint {}: {}", name, e);
            }
        }

        Ok(())
    }

    async fn init_remote_endpoint(&self, config: EndpointConfig) -> Result<()> {
        let name = config.name.clone();

        self.registry
            .register(name.clone(), name.clone(), EndpointType::Remote)?;

        let remote_endpoint = RemoteEndpoint::from_config(&config)?;
        let endpoint_kind = EndpointKind::Remote(remote_endpoint);
        self.endpoints
            .insert(name.clone(), Arc::new(RwLock::new(endpoint_kind)));

        info!("Registered remote endpoint: {} at path /{}", name, name);

        Ok(())
    }

    /// Start an MCP endpoint (works for both local and remote)
    pub(crate) async fn start_endpoint(&self, name: &str) -> Result<()> {
        let info = self.registry.get(name)?;

        if info.status == EndpointStatus::Running {
            return Err(ProxyError::ServerAlreadyRunning(name.to_string()));
        }

        self.registry.set_status(name, EndpointStatus::Starting)?;

        let endpoint_lock = self
            .endpoints
            .get(name)
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))?;

        let mut endpoint = endpoint_lock.write().await;

        match endpoint.start().await {
            Ok(()) => {
                self.registry.set_status(name, EndpointStatus::Running)?;
                info!("Successfully started endpoint: {}", name);
                Ok(())
            }
            Err(e) => {
                self.registry.set_status(name, EndpointStatus::Failed)?;
                error!("Failed to start endpoint {}: {}", name, e);
                Err(e)
            }
        }
    }

    /// Stop an MCP endpoint (works for both local and remote)
    pub(crate) async fn stop_endpoint(&self, name: &str) -> Result<()> {
        let info = self.registry.get(name)?;

        if info.status == EndpointStatus::Stopped {
            return Err(ProxyError::ServerNotRunning(name.to_string()));
        }

        self.registry.set_status(name, EndpointStatus::Stopping)?;

        let endpoint_lock = self
            .endpoints
            .get(name)
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))?;

        let mut endpoint = endpoint_lock.write().await;

        match endpoint.stop().await {
            Ok(()) => {
                self.registry.set_status(name, EndpointStatus::Stopped)?;
                info!("Successfully stopped endpoint: {}", name);
                Ok(())
            }
            Err(e) => {
                error!("Failed to stop endpoint {}: {}", name, e);
                Err(e)
            }
        }
    }

    /// Restart an MCP endpoint
    pub(crate) async fn restart_endpoint(&self, name: &str) -> Result<()> {
        info!("Restarting endpoint: {}", name);
        self.stop_endpoint(name).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start_endpoint(name).await?;
        Ok(())
    }

    /// Get endpoint info by name
    pub(crate) fn get_endpoint_info(&self, name: &str) -> Result<EndpointInfo> {
        self.registry.get(name)
    }

    /// List all registered endpoints
    pub(crate) fn list_endpoints(&self) -> Vec<EndpointInfo> {
        self.registry.list()
    }

    /// Get an endpoint instance by name (polymorphic access)
    pub(crate) fn get_endpoint(&self, name: &str) -> Result<Arc<RwLock<EndpointKind>>> {
        self.endpoints
            .get(name)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))
    }

    /// Get an MCP client for any endpoint (works for both local and remote)
    pub(crate) async fn get_client(&self, name: &str) -> Result<Arc<McpClient>> {
        let endpoint = self.get_endpoint(name)?;
        let endpoint_guard = endpoint.read().await;
        endpoint_guard.get_or_create_client().await
    }

    /// Shutdown all endpoints
    pub(crate) async fn shutdown(&self) -> Result<()> {
        info!("Shutting down all endpoints");

        for entry in self.endpoints.iter() {
            let name = entry.key();
            
            // Only stop local endpoints; remote endpoints are external services
            // that don't need lifecycle management
            if let Ok(info) = self.registry.get(name) {
                if info.endpoint_type == EndpointType::Local {
                    if let Err(e) = self.stop_endpoint(name).await {
                        warn!("Error stopping endpoint {} during shutdown: {}", name, e);
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for EndpointManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EndpointKindConfig;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_init_local_endpoint_no_autostart() {
        let manager = EndpointManager::new();

        let config = EndpointConfig {
            name: "test-server".to_string(),
            endpoint_type: EndpointKindConfig::Local {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                env: HashMap::new(),
                auto_start: false,
                restart_on_failure: false,
            },
            tools: None,
        };

        manager.init_from_config(vec![config]).await.unwrap();

        let info = manager.get_endpoint_info("test-server").unwrap();
        assert_eq!(info.status, EndpointStatus::Stopped);
    }

    #[tokio::test]
    async fn test_start_endpoint_fails_with_non_mcp_process() {
        let manager = EndpointManager::new();

        let config = EndpointConfig {
            name: "test-echo".to_string(),
            endpoint_type: EndpointKindConfig::Local {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                env: HashMap::new(),
                auto_start: false,
                restart_on_failure: false,
            },
            tools: None,
        };

        manager.init_from_config(vec![config]).await.unwrap();

        let result = manager.start_endpoint("test-echo").await;
        assert!(result.is_err(), "start should fail for non-MCP process");

        let info = manager.get_endpoint_info("test-echo").unwrap();
        assert_eq!(info.status, EndpointStatus::Failed);
    }

    #[tokio::test]
    async fn test_remote_endpoint_registration() {
        let manager = EndpointManager::new();

        let config = EndpointConfig {
            name: "remote-server".to_string(),
            endpoint_type: EndpointKindConfig::Remote {
                url: "https://example.com".to_string(),
            },
            tools: None,
        };

        manager.init_from_config(vec![config]).await.unwrap();

        let info = manager.get_endpoint_info("remote-server").unwrap();
        assert!(matches!(info.endpoint_type, EndpointType::Remote));

        let result = manager.start_endpoint("remote-server").await;
        assert!(result.is_err());
    }
}
