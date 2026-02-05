use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(flatten)]
    pub server_type: McpServerType,
    #[serde(default)]
    pub tools: Option<ToolFilter>,
    pub path: Option<String>, // URL path, defaults to name if not set
}

impl McpServerConfig {
    /// Get the URL path for this server (defaults to name if not specified)
    pub fn get_path(&self) -> String {
        self.path.clone().unwrap_or_else(|| self.name.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerType {
    Local {
        command: String,
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default = "default_auto_start")]
        auto_start: bool,
        #[serde(default)]
        restart_on_failure: bool,
    },
    Remote {
        url: String,
    },
}

fn default_auto_start() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolFilter {
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

impl ToolFilter {
    /// Check if a tool should be allowed based on include/exclude filters
    /// Include list takes precedence - if present, tool must be in it
    /// Exclude list is then checked - if present, tool must not be in it
    pub fn should_allow(&self, tool_name: &str) -> bool {
        // If include list exists, tool must be in it
        if let Some(include) = &self.include {
            if !include.iter().any(|t| t == tool_name) {
                return false;
            }
        }

        // If exclude list exists, tool must not be in it
        if let Some(exclude) = &self.exclude {
            if exclude.iter().any(|t| t == tool_name) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_filter_include_only() {
        let filter = ToolFilter {
            include: Some(vec!["tool1".to_string(), "tool2".to_string()]),
            exclude: None,
        };

        assert!(filter.should_allow("tool1"));
        assert!(filter.should_allow("tool2"));
        assert!(!filter.should_allow("tool3"));
    }

    #[test]
    fn test_tool_filter_exclude_only() {
        let filter = ToolFilter {
            include: None,
            exclude: Some(vec!["tool1".to_string()]),
        };

        assert!(!filter.should_allow("tool1"));
        assert!(filter.should_allow("tool2"));
        assert!(filter.should_allow("tool3"));
    }

    #[test]
    fn test_tool_filter_include_and_exclude() {
        let filter = ToolFilter {
            include: Some(vec![
                "tool1".to_string(),
                "tool2".to_string(),
                "tool3".to_string(),
            ]),
            exclude: Some(vec!["tool2".to_string()]),
        };

        assert!(filter.should_allow("tool1"));
        assert!(!filter.should_allow("tool2")); // excluded even though in include
        assert!(filter.should_allow("tool3"));
        assert!(!filter.should_allow("tool4")); // not in include list
    }

    #[test]
    fn test_tool_filter_no_filters() {
        let filter = ToolFilter {
            include: None,
            exclude: None,
        };

        assert!(filter.should_allow("tool1"));
        assert!(filter.should_allow("tool2"));
        assert!(filter.should_allow("anything"));
    }
}
