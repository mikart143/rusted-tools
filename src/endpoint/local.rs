use crate::config::LocalEndpointSettings;
use crate::endpoint::HttpTransportAdapter;
use crate::endpoint::client_holder::ClientHolder;
use crate::error::Result;
use crate::mcp::McpClient;
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
        let client_holder = ClientHolder::new(name.clone());
        Self {
            name,
            config,
            client_holder,
        }
    }

    pub(crate) async fn get_client(&self) -> Result<Arc<McpClient>> {
        let client = self.client_holder.get();
        if client.is_running().await {
            Ok(client)
        } else {
            Err(crate::error::ProxyError::server_not_running(
                self.name.clone(),
            ))
        }
    }
}

impl LocalEndpoint {
    pub(crate) async fn start(&mut self) -> Result<()> {
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
            crate::error::ProxyError::server_start_failed(&self.name, e)
        })?;

        let client = self.client_holder.get();
        client.init_with_transport(transport).await?;

        info!("Successfully started local MCP endpoint: {}", self.name);
        Ok(())
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        info!("Stopping local MCP endpoint: {}", self.name);

        let client = self.client_holder.get();
        client.stop().await?;

        info!("Successfully stopped local MCP endpoint: {}", self.name);
        Ok(())
    }

    pub(crate) async fn get_or_create_client(&self) -> Result<Arc<McpClient>> {
        self.get_client().await
    }
}

impl HttpTransportAdapter for LocalEndpoint {
    fn attach_http_route<S>(
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

        let client = self.client_holder.get();
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
        };

        let mut endpoint = LocalEndpoint::new("test-echo".to_string(), config);

        let start_result = endpoint.start().await;
        assert!(
            start_result.is_err(),
            "start() should fail for non-MCP process"
        );
    }

    #[tokio::test]
    async fn test_process_exit_behavior() {
        let config = LocalEndpointSettings {
            command: "true".to_string(),
            args: vec![],
            env: HashMap::new(),
        };

        let mut endpoint = LocalEndpoint::new("test-exit".to_string(), config);

        let result = endpoint.start().await;
        assert!(
            result.is_err(),
            "start() should fail when process exits immediately"
        );
    }
}
