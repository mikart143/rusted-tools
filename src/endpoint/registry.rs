use crate::error::{ProxyError, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// Status of an MCP endpoint instance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum EndpointStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

impl fmt::Display for EndpointStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EndpointStatus::Starting => "starting",
            EndpointStatus::Running => "running",
            EndpointStatus::Stopping => "stopping",
            EndpointStatus::Stopped => "stopped",
            EndpointStatus::Failed => "failed",
        };
        write!(f, "{}", s)
    }
}

/// Information about a registered endpoint
#[derive(Debug, Clone)]
pub(crate) struct EndpointInfo {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) endpoint_type: EndpointType,
    pub(crate) status: EndpointStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum EndpointType {
    Local,
    Remote,
}

impl fmt::Display for EndpointType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EndpointType::Local => "local",
            EndpointType::Remote => "remote",
        };
        write!(f, "{}", s)
    }
}

/// Registry for tracking active MCP endpoint instances
#[derive(Clone)]
pub(crate) struct EndpointRegistry {
    endpoints: Arc<DashMap<String, EndpointInfo>>,
}

impl EndpointRegistry {
    pub(crate) fn new() -> Self {
        Self {
            endpoints: Arc::new(DashMap::new()),
        }
    }

    /// Register a new endpoint
    pub(crate) fn register(
        &self,
        name: String,
        path: String,
        endpoint_type: EndpointType,
    ) -> Result<()> {
        if self.endpoints.contains_key(&name) {
            return Err(ProxyError::ServerAlreadyExists(name));
        }

        let info = EndpointInfo {
            name: name.clone(),
            path,
            endpoint_type,
            status: EndpointStatus::Stopped,
        };

        self.endpoints.insert(name, info);
        Ok(())
    }

    /// Get endpoint info by name
    pub(crate) fn get(&self, name: &str) -> Result<EndpointInfo> {
        self.endpoints
            .get(name)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))
    }

    /// Update endpoint status
    pub(crate) fn set_status(&self, name: &str, status: EndpointStatus) -> Result<()> {
        let mut entry = self
            .endpoints
            .get_mut(name)
            .ok_or_else(|| ProxyError::ServerNotFound(name.to_string()))?;
        entry.status = status;
        Ok(())
    }

    /// List all registered endpoints
    pub(crate) fn list(&self) -> Vec<EndpointInfo> {
        self.endpoints
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
}

impl Default for EndpointRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let registry = EndpointRegistry::new();
        registry
            .register(
                "test-server".to_string(),
                "test".to_string(),
                EndpointType::Local,
            )
            .unwrap();

        let info = registry.get("test-server").unwrap();
        assert_eq!(info.name, "test-server");
        assert_eq!(info.path, "test");
        assert_eq!(info.status, EndpointStatus::Stopped);
    }

    #[test]
    fn test_duplicate_registration() {
        let registry = EndpointRegistry::new();
        registry
            .register(
                "test-server".to_string(),
                "test".to_string(),
                EndpointType::Local,
            )
            .unwrap();

        let result = registry.register(
            "test-server".to_string(),
            "test2".to_string(),
            EndpointType::Local,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_set_status() {
        let registry = EndpointRegistry::new();
        registry
            .register(
                "test-server".to_string(),
                "test".to_string(),
                EndpointType::Local,
            )
            .unwrap();

        registry
            .set_status("test-server", EndpointStatus::Running)
            .unwrap();
        let info = registry.get("test-server").unwrap();
        assert_eq!(info.status, EndpointStatus::Running);
    }

    #[test]
    fn test_list() {
        let registry = EndpointRegistry::new();
        registry
            .register(
                "server1".to_string(),
                "path1".to_string(),
                EndpointType::Local,
            )
            .unwrap();
        registry
            .register(
                "server2".to_string(),
                "path2".to_string(),
                EndpointType::Remote,
            )
            .unwrap();

        let endpoints = registry.list();
        assert_eq!(endpoints.len(), 2);
    }
}
