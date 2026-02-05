use crate::config::McpServerConfig;
use crate::error::{ProxyError, Result};
use tracing::info;

/// Represents a remote MCP server accessed via HTTP/SSE
#[derive(Debug, Clone)]
pub struct RemoteMcpServer {
    pub name: String,
    pub url: String,
}

impl RemoteMcpServer {
    pub fn new(name: String, url: String) -> Self {
        Self { name, url }
    }

    /// Create from configuration
    pub fn from_config(config: &McpServerConfig) -> Result<Self> {
        match &config.server_type {
            crate::config::McpServerType::Remote { url } => {
                info!("Configured remote MCP server: {} at {}", config.name, url);
                Ok(Self::new(config.name.clone(), url.clone()))
            }
            _ => Err(ProxyError::Config(
                "Expected remote server configuration".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpServerType;

    #[test]
    fn test_create_remote_server() {
        let config = McpServerConfig {
            name: "test-remote".to_string(),
            server_type: McpServerType::Remote {
                url: "https://example.com".to_string(),
            },
            tools: None,
            path: Some("remote".to_string()),
        };

        let server = RemoteMcpServer::from_config(&config).unwrap();
        assert_eq!(server.name, "test-remote");
        assert_eq!(server.url, "https://example.com");
    }

    #[test]
    fn test_from_config_with_local_config_fails() {
        let config = McpServerConfig {
            name: "test-local".to_string(),
            server_type: McpServerType::Local {
                command: "echo".to_string(),
                args: vec![],
                env: Default::default(),
                auto_start: false,
                restart_on_failure: false,
            },
            tools: None,
            path: Some("local".to_string()),
        };

        let result = RemoteMcpServer::from_config(&config);
        assert!(result.is_err());
    }
}
