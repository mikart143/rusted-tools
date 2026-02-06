use crate::config::EndpointConfig;
use crate::endpoint::registry::EndpointType;
use crate::endpoint::traits::EndpointInstance;
use crate::error::{ProxyError, Result};
use crate::mcp::McpClient;
use async_trait::async_trait;
use axum::Router;
use axum_reverse_proxy::ReverseProxy;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Represents a remote MCP endpoint accessed via HTTP/SSE
#[derive(Clone)]
pub struct RemoteEndpoint {
    pub name: String,
    pub path: String,
    pub url: String,
    mcp_client: Arc<RwLock<Option<Arc<McpClient>>>>,
}

impl RemoteEndpoint {
    pub fn new(name: String, path: String, url: String) -> Self {
        Self {
            name,
            path,
            url,
            mcp_client: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration
    pub fn from_config(config: &EndpointConfig) -> Result<Self> {
        match &config.endpoint_type {
            crate::config::EndpointKindConfig::Remote { url } => {
                let path = config.path.clone().unwrap_or_else(|| config.name.clone());
                info!(
                    "Configured remote MCP endpoint: {} at {} (path: {})",
                    config.name, url, path
                );
                Ok(Self::new(config.name.clone(), path, url.clone()))
            }
            _ => Err(ProxyError::Config(
                "Expected remote endpoint configuration".to_string(),
            )),
        }
    }
}

#[async_trait]
impl EndpointInstance for RemoteEndpoint {
    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn endpoint_type(&self) -> EndpointType {
        EndpointType::Remote
    }

    async fn start(&mut self) -> Result<()> {
        if self.mcp_client.read().await.is_some() {
            return Err(ProxyError::ServerAlreadyRunning(self.name.clone()));
        }

        info!(
            "Starting remote MCP endpoint: {} at {}",
            self.name, self.url
        );

        let client = McpClient::new(self.name.clone());
        client.init_with_http(&self.url).await?;

        match client.list_tools().await {
            Ok(tools) => {
                info!(
                    "Successfully connected to remote endpoint {} ({} tools available)",
                    self.name,
                    tools.len()
                );
            }
            Err(e) => {
                warn!(
                    "Connected to remote endpoint {} but failed to list tools: {}",
                    self.name, e
                );
            }
        }

        let mut client_lock = self.mcp_client.write().await;
        *client_lock = Some(Arc::new(client));

        info!("Successfully started remote MCP endpoint: {}", self.name);
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if self.mcp_client.read().await.is_none() {
            return Err(ProxyError::ServerNotRunning(self.name.clone()));
        }

        info!("Stopping remote MCP endpoint: {}", self.name);

        let mut client_lock = self.mcp_client.write().await;
        *client_lock = None;

        info!("Successfully stopped remote MCP endpoint: {}", self.name);
        Ok(())
    }

    async fn get_or_create_client(&self) -> Result<Arc<McpClient>> {
        let client_lock = self.mcp_client.read().await;
        if let Some(client) = client_lock.as_ref() {
            return Ok(client.clone());
        }
        drop(client_lock);

        info!(
            "Creating new HTTP client for remote endpoint: {}",
            self.name
        );
        let client = McpClient::new(self.name.clone());
        client.init_with_http(&self.url).await?;

        Ok(Arc::new(client))
    }

    fn is_started(&self) -> bool {
        self.mcp_client
            .try_read()
            .map(|lock| lock.is_some())
            .unwrap_or(false)
    }

    async fn attach_http_route<S>(
        &self,
        router: Router<S>,
        path: &str,
        _ct: CancellationToken,
    ) -> Result<Router<S>>
    where
        S: Clone + Send + Sync + 'static,
    {
        info!(
            "Setting up HTTP reverse proxy for remote endpoint {} at /mcp/{} → {}",
            self.name, path, self.url
        );

        let proxy = ReverseProxy::new(&format!("/mcp/{}", path), &self.url);

        Ok(router.merge(proxy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EndpointKindConfig;

    #[test]
    fn test_create_remote_endpoint() {
        let config = EndpointConfig {
            name: "test-remote".to_string(),
            endpoint_type: EndpointKindConfig::Remote {
                url: "https://example.com".to_string(),
            },
            tools: None,
            path: Some("remote".to_string()),
        };

        let endpoint = RemoteEndpoint::from_config(&config).unwrap();
        assert_eq!(endpoint.name, "test-remote");
        assert_eq!(endpoint.path, "remote");
        assert_eq!(endpoint.url, "https://example.com");
    }

    #[test]
    fn test_from_config_with_local_config_fails() {
        let config = EndpointConfig {
            name: "test-local".to_string(),
            endpoint_type: EndpointKindConfig::Local {
                command: "echo".to_string(),
                args: vec![],
                env: Default::default(),
                auto_start: false,
                restart_on_failure: false,
            },
            tools: None,
            path: Some("local".to_string()),
        };

        let result = RemoteEndpoint::from_config(&config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_started() {
        let endpoint = RemoteEndpoint::new(
            "test".to_string(),
            "test-path".to_string(),
            "http://localhost:8080".to_string(),
        );

        assert!(!endpoint.is_started());
    }
}
