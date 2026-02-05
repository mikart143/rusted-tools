use crate::error::{ProxyError, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// Status of an MCP server instance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

impl fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ServerStatus::Starting => "starting",
            ServerStatus::Running => "running",
            ServerStatus::Stopping => "stopping",
            ServerStatus::Stopped => "stopped",
            ServerStatus::Failed => "failed",
        };
        write!(f, "{}", s)
    }
}

/// Information about a registered server
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub path: String,
    pub server_type: ServerType,
    pub status: ServerStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerType {
    Local,
    Remote,
}

impl fmt::Display for ServerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ServerType::Local => "local",
            ServerType::Remote => "remote",
        };
        write!(f, "{}", s)
    }
}

/// Registry for tracking active MCP server instances
#[derive(Clone)]
pub struct ServerRegistry {
    servers: Arc<DashMap<String, ServerInfo>>,
}

impl ServerRegistry {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(DashMap::new()),
        }
    }

    /// Register a new server
    pub fn register(&self, name: String, path: String, server_type: ServerType) -> Result<()> {
        if self.servers.contains_key(&name) {
            return Err(ProxyError::ServerAlreadyExists(name));
        }

        let info = ServerInfo {
            name: name.clone(),
            path,
            server_type,
            status: ServerStatus::Stopped,
        };

        self.servers.insert(name, info);
        Ok(())
    }

    /// Get server info by name
    pub fn get(&self, name: &str) -> Result<ServerInfo> {
        self.servers
            .get(name)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))
    }

    /// Update server status
    pub fn set_status(&self, name: &str, status: ServerStatus) -> Result<()> {
        let mut entry = self
            .servers
            .get_mut(name)
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))?;
        entry.status = status;
        Ok(())
    }

    /// List all registered servers
    pub fn list(&self) -> Vec<ServerInfo> {
        self.servers
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
}

impl Default for ServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let registry = ServerRegistry::new();
        registry
            .register(
                "test-server".to_string(),
                "test".to_string(),
                ServerType::Local,
            )
            .unwrap();

        let info = registry.get("test-server").unwrap();
        assert_eq!(info.name, "test-server");
        assert_eq!(info.path, "test");
        assert_eq!(info.status, ServerStatus::Stopped);
    }

    #[test]
    fn test_duplicate_registration() {
        let registry = ServerRegistry::new();
        registry
            .register(
                "test-server".to_string(),
                "test".to_string(),
                ServerType::Local,
            )
            .unwrap();

        let result = registry.register(
            "test-server".to_string(),
            "test2".to_string(),
            ServerType::Local,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_set_status() {
        let registry = ServerRegistry::new();
        registry
            .register(
                "test-server".to_string(),
                "test".to_string(),
                ServerType::Local,
            )
            .unwrap();

        registry
            .set_status("test-server", ServerStatus::Running)
            .unwrap();
        let info = registry.get("test-server").unwrap();
        assert_eq!(info.status, ServerStatus::Running);
    }

    #[test]
    fn test_list() {
        let registry = ServerRegistry::new();
        registry
            .register(
                "server1".to_string(),
                "path1".to_string(),
                ServerType::Local,
            )
            .unwrap();
        registry
            .register(
                "server2".to_string(),
                "path2".to_string(),
                ServerType::Remote,
            )
            .unwrap();

        let servers = registry.list();
        assert_eq!(servers.len(), 2);
    }
}
