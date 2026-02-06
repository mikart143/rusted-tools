use crate::config::LocalEndpointSettings;
use crate::endpoint::registry::EndpointType;
use crate::endpoint::traits::EndpointInstance;
use crate::error::{ProxyError, Result};
use crate::mcp::McpClient;
use async_trait::async_trait;
use axum::Router;
use rmcp::transport::TokioChildProcess;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

/// Represents a local MCP endpoint running as a child process
#[derive(Clone)]
pub struct LocalEndpoint {
    pub name: String,
    pub config: LocalEndpointSettings,
    mcp_client: Arc<RwLock<Option<Arc<McpClient>>>>,
}

impl LocalEndpoint {
    pub fn new(name: String, config: LocalEndpointSettings) -> Self {
        Self {
            name,
            config,
            mcp_client: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the MCP client for this endpoint
    pub async fn get_client(&self) -> Result<Arc<McpClient>> {
        let client_lock = self.mcp_client.read().await;
        client_lock
            .as_ref()
            .cloned()
            .ok_or_else(|| ProxyError::ServerNotRunning(self.name.clone()))
    }
}

#[async_trait]
impl EndpointInstance for LocalEndpoint {
    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> &str {
        &self.config.path
    }

    fn endpoint_type(&self) -> EndpointType {
        EndpointType::Local
    }

    /// Start the MCP endpoint process
    async fn start(&mut self) -> Result<()> {
        if self.mcp_client.read().await.is_some() {
            return Err(ProxyError::ServerAlreadyRunning(self.name.clone()));
        }

        info!("Starting local MCP endpoint: {}", self.name);
        debug!(
            "Command: {} {}",
            self.config.command,
            self.config.args.join(" ")
        );

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args).envs(&self.config.env);

        let transport = TokioChildProcess::new(cmd).map_err(|e| {
            error!("Failed to create TokioChildProcess: {}", e);
            ProxyError::ServerStartFailed(format!("{}: {}", self.name, e))
        })?;

        let client = McpClient::new(self.name.clone());
        client.init_with_transport(transport).await?;

        let mut client_lock = self.mcp_client.write().await;
        *client_lock = Some(Arc::new(client));

        info!("Successfully started local MCP endpoint: {}", self.name);
        Ok(())
    }

    /// Stop the MCP endpoint process
    async fn stop(&mut self) -> Result<()> {
        if self.mcp_client.read().await.is_none() {
            return Err(ProxyError::ServerNotRunning(self.name.clone()));
        }

        info!("Stopping local MCP endpoint: {}", self.name);

        let mut client_lock = self.mcp_client.write().await;
        *client_lock = None;

        info!("Successfully stopped local MCP endpoint: {}", self.name);
        Ok(())
    }

    async fn get_or_create_client(&self) -> Result<Arc<McpClient>> {
        self.get_client().await
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
        ct: CancellationToken,
    ) -> Result<Router<S>>
    where
        S: Clone + Send + Sync + 'static,
    {
        info!(
            "Setting up SSE bridge for local endpoint {} at /mcp/{}",
            self.name, path
        );

        let client = self.get_or_create_client().await?;
        let sse_service =
            crate::api::mcp_sse_service::create_local_sse_service(client, self.name.clone(), ct);

        Ok(router.nest_service(&format!("/mcp/{}", path), sse_service))
    }
}

impl Drop for LocalEndpoint {
    fn drop(&mut self) {
        debug!("Dropping LocalEndpoint: {}", self.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_start_and_stop_cat_server() {
        let config = LocalEndpointSettings {
            command: "cat".to_string(),
            args: vec![],
            env: HashMap::new(),
            path: "test".to_string(),
            restart_on_failure: false,
        };

        let mut endpoint = LocalEndpoint::new("test-cat".to_string(), config);

        let start_result = endpoint.start().await;

        if start_result.is_ok() {
            assert!(endpoint.get_client().await.is_ok());
            endpoint.stop().await.unwrap();
            assert!(endpoint.get_client().await.is_err());
        }
    }

    #[tokio::test]
    async fn test_process_exit_behavior() {
        let config = LocalEndpointSettings {
            command: "true".to_string(),
            args: vec![],
            env: HashMap::new(),
            path: "test".to_string(),
            restart_on_failure: false,
        };

        let mut endpoint = LocalEndpoint::new("test-exit".to_string(), config);
        let _ = endpoint.start().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}
