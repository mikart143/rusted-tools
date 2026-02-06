use crate::config::LocalEndpointSettings;
use crate::endpoint::client_holder::ClientHolder;
use crate::endpoint::registry::EndpointType;
use crate::endpoint::traits::EndpointInstance;
use crate::error::Result;
use crate::mcp::McpClient;
use async_trait::async_trait;
use axum::Router;
use rmcp::transport::TokioChildProcess;
use std::sync::Arc;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

/// Represents a local MCP endpoint running as a child process
#[derive(Clone)]
pub(crate) struct LocalEndpoint {
    pub(crate) name: String,
    pub(crate) config: LocalEndpointSettings,
    client_holder: ClientHolder,
}

impl LocalEndpoint {
    pub(crate) fn new(name: String, config: LocalEndpointSettings) -> Self {
        Self {
            name,
            config,
            client_holder: ClientHolder::new(),
        }
    }

    pub(crate) async fn get_client(&self) -> Result<Arc<McpClient>> {
        self.client_holder.get(&self.name).await
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

    async fn start(&mut self) -> Result<()> {
        self.client_holder.ensure_not_running(&self.name).await?;

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
            crate::error::ProxyError::ServerStartFailed(format!("{}: {}", self.name, e))
        })?;

        let client = McpClient::new(self.name.clone());
        client.init_with_transport(transport).await?;

        self.client_holder.set(client).await;

        info!("Successfully started local MCP endpoint: {}", self.name);
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.client_holder.ensure_running(&self.name).await?;

        info!("Stopping local MCP endpoint: {}", self.name);

        self.client_holder.clear().await;

        info!("Successfully stopped local MCP endpoint: {}", self.name);
        Ok(())
    }

    async fn get_or_create_client(&self) -> Result<Arc<McpClient>> {
        self.get_client().await
    }

    fn is_started(&self) -> bool {
        self.client_holder.is_set()
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
    async fn test_start_fails_with_non_mcp_process() {
        let config = LocalEndpointSettings {
            command: "echo".to_string(),
            args: vec!["not-an-mcp-server".to_string()],
            env: HashMap::new(),
            path: "test".to_string(),
            restart_on_failure: false,
        };

        let mut endpoint = LocalEndpoint::new("test-echo".to_string(), config);

        let start_result = endpoint.start().await;
        assert!(
            start_result.is_err(),
            "start() should fail for non-MCP process"
        );
        assert!(!endpoint.is_started());
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

        let result = endpoint.start().await;
        assert!(
            result.is_err(),
            "start() should fail when process exits immediately"
        );
        assert!(!endpoint.is_started());
    }
}
