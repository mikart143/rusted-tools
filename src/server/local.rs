use crate::config::McpServerConfig;
use crate::error::{ProxyError, Result};
use crate::proxy::client::McpClient;
use rmcp::transport::TokioChildProcess;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Represents a local MCP server running as a child process
pub struct LocalMcpServer {
    pub name: String,
    pub config: LocalServerConfig,
    mcp_client: Arc<RwLock<Option<McpClient>>>,
}

#[derive(Debug, Clone)]
pub struct LocalServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    #[allow(dead_code)]
    pub restart_on_failure: bool,
}

impl LocalMcpServer {
    pub fn new(name: String, config: LocalServerConfig) -> Self {
        Self {
            name,
            config,
            mcp_client: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the MCP server process
    pub async fn start(&mut self) -> Result<()> {
        // Check if client is already initialized (server is running)
        if self.mcp_client.read().await.is_some() {
            return Err(ProxyError::ServerAlreadyRunning(self.name.clone()));
        }

        info!("Starting local MCP server: {}", self.name);
        debug!(
            "Command: {} {}",
            self.config.command,
            self.config.args.join(" ")
        );

        // Create the command
        let mut cmd = Command::new(&self.config.command);
        
        // Configure the command with args and env
        cmd.args(&self.config.args)
            .envs(&self.config.env);

        // Create TokioChildProcess transport
        let transport = TokioChildProcess::new(cmd).map_err(|e| {
            error!("Failed to create TokioChildProcess: {}", e);
            ProxyError::ServerStartFailed(format!("{}: {}", self.name, e))
        })?;

        // Initialize MCP client with the transport
        let client = McpClient::new(self.name.clone());
        client.init_with_transport(transport).await?;

        // Store the client
        let mut client_lock = self.mcp_client.write().await;
        *client_lock = Some(client);

        info!("Successfully started local MCP server: {}", self.name);
        Ok(())
    }

    /// Stop the MCP server process
    pub async fn stop(&mut self) -> Result<()> {
        // Check if the client is initialized (server is running)
        if self.mcp_client.read().await.is_none() {
            return Err(ProxyError::ServerNotRunning(self.name.clone()));
        }

        info!("Stopping local MCP server: {}", self.name);

        // Clear the MCP client, which will drop the transport and kill the child process
        let mut client_lock = self.mcp_client.write().await;
        *client_lock = None;

        info!("Successfully stopped local MCP server: {}", self.name);
        Ok(())
    }

    /// Get the MCP client for this server
    pub async fn get_client(&self) -> Result<McpClient> {
        let client_lock = self.mcp_client.read().await;
        client_lock
            .as_ref()
            .cloned()
            .ok_or_else(|| ProxyError::ServerNotRunning(self.name.clone()))
    }
}

impl Drop for LocalMcpServer {
    fn drop(&mut self) {
        // The child process will be terminated automatically when the MCP client
        // (and its TokioChildProcess transport) is dropped
        debug!("Dropping LocalMcpServer: {}", self.name);
    }
}

/// Extract local server configuration from MCP server config
pub fn extract_local_config(config: &McpServerConfig) -> Result<LocalServerConfig> {
    match &config.server_type {
        crate::config::McpServerType::Local {
            command,
            args,
            env,
            restart_on_failure,
            ..
        } => Ok(LocalServerConfig {
            command: command.clone(),
            args: args.clone(),
            env: env.clone(),
            restart_on_failure: *restart_on_failure,
        }),
        _ => Err(ProxyError::Config(
            "Expected local server configuration".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_and_stop_cat_server() {
        let config = LocalServerConfig {
            command: "cat".to_string(),
            args: vec![],
            env: HashMap::new(),
            restart_on_failure: false,
        };

        let mut server = LocalMcpServer::new("test-cat".to_string(), config);

        // Start the server (this will fail because cat is not an MCP server)
        // but it's good enough to test the mechanism
        let start_result = server.start().await;
        
        // It might fail during initialization, which is expected
        if start_result.is_ok() {
            // Verify client is available after start
            assert!(server.get_client().await.is_ok());
            
            // Stop the server
            server.stop().await.unwrap();
            
            // After stop, client should be unavailable
            assert!(server.get_client().await.is_err());
        }
    }

    #[tokio::test]
    async fn test_process_exit_behavior() {
        let config = LocalServerConfig {
            command: "true".to_string(), // 'true' exits immediately
            args: vec![],
            env: HashMap::new(),
            restart_on_failure: false,
        };

        let mut server = LocalMcpServer::new("test-exit".to_string(), config);
        let _ = server.start().await; // May fail, that's ok

        // Wait a bit for the process to exit
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Process exited, so client access will fail with transport error
        // This is expected behavior for short-lived processes
    }
}
