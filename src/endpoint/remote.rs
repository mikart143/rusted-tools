use crate::config::EndpointConfig;
use crate::endpoint::HttpTransportAdapter;
use crate::endpoint::client_holder::ClientHolder;
use crate::error::{ProxyError, Result};
use crate::mcp::McpClient;
use axum::Router;
use axum_reverse_proxy::ReverseProxy;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Represents a remote MCP endpoint accessed via HTTP/SSE
#[derive(Clone)]
pub(crate) struct RemoteEndpoint {
    pub(crate) name: String,
    pub(crate) url: String,
    client_holder: ClientHolder,
}

impl RemoteEndpoint {
    pub(crate) fn new(name: String, url: String) -> Self {
        let client_holder = ClientHolder::new(name.clone());
        Self {
            name,
            url,
            client_holder,
        }
    }

    pub(crate) fn from_config(config: &EndpointConfig) -> Result<Self> {
        match &config.endpoint_type {
            crate::config::EndpointKindConfig::Remote { url } => {
                info!("Configured remote MCP endpoint: {} at {}", config.name, url);
                Ok(Self::new(config.name.clone(), url.clone()))
            }
            _ => Err(ProxyError::config(
                "Expected remote endpoint configuration",
            )),
        }
    }
}

impl RemoteEndpoint {
    pub(crate) async fn start(&mut self) -> Result<()> {
        info!(
            "Starting remote MCP endpoint: {} at {}",
            self.name, self.url
        );

        let client = self.client_holder.get();
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

        info!("Successfully started remote MCP endpoint: {}", self.name);
        Ok(())
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        info!("Stopping remote MCP endpoint: {}", self.name);

        let client = self.client_holder.get();
        client.stop().await?;

        info!("Successfully stopped remote MCP endpoint: {}", self.name);
        Ok(())
    }

    pub(crate) async fn get_or_create_client(&self) -> Result<Arc<McpClient>> {
        let client = self.client_holder.get();
        if !client.is_running().await {
            info!(
                "Creating new HTTP client for remote endpoint: {}",
                self.name
            );
            client.init_with_http(&self.url).await?;
        }

        Ok(client)
    }
}

impl HttpTransportAdapter for RemoteEndpoint {
    fn attach_http_route<S>(
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
        };

        let endpoint = RemoteEndpoint::from_config(&config).unwrap();
        assert_eq!(endpoint.name, "test-remote");
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
            },
            tools: None,
        };

        let result = RemoteEndpoint::from_config(&config);
        assert!(result.is_err());
    }
}
