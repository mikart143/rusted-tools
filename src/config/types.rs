use crate::error::{ProxyError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub http: HttpConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub endpoints: Vec<EndpointConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HttpConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
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
pub struct McpConfig {
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_restart_delay_ms")]
    pub restart_delay_ms: u64,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            request_timeout_secs: default_request_timeout_secs(),
            restart_delay_ms: default_restart_delay_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EndpointConfig {
    pub name: String,
    #[serde(flatten)]
    pub endpoint_type: EndpointKindConfig,
    #[serde(default)]
    pub tools: Option<ToolFilter>,
}

impl EndpointConfig {
    /// Extract local endpoint settings from this config
    pub(crate) fn to_local_settings(&self) -> Result<LocalEndpointSettings> {
        match &self.endpoint_type {
            EndpointKindConfig::Local {
                command, args, env, ..
            } => Ok(LocalEndpointSettings {
                command: command.clone(),
                args: args.clone(),
                env: env.clone(),
            }),
            _ => Err(ProxyError::Config(
                "Expected local endpoint configuration".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum EndpointKindConfig {
    Local {
        command: String,
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default = "default_auto_start")]
        auto_start: bool,
    },
    Remote {
        url: String,
    },
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "pretty".to_string()
}

fn default_auto_start() -> bool {
    true
}

fn default_request_timeout_secs() -> u64 {
    30
}

fn default_restart_delay_ms() -> u64 {
    500
}

/// Local endpoint settings extracted from config
#[derive(Debug, Clone)]
pub(crate) struct LocalEndpointSettings {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolFilter {
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_filter_include_only() {
        use crate::routing::tool_filter::is_tool_allowed;
        let filter = ToolFilter {
            include: Some(vec!["tool1".to_string(), "tool2".to_string()]),
            exclude: None,
        };

        assert!(is_tool_allowed("tool1", Some(&filter)));
        assert!(is_tool_allowed("tool2", Some(&filter)));
        assert!(!is_tool_allowed("tool3", Some(&filter)));
    }

    #[test]
    fn test_tool_filter_exclude_only() {
        use crate::routing::tool_filter::is_tool_allowed;
        let filter = ToolFilter {
            include: None,
            exclude: Some(vec!["tool1".to_string()]),
        };

        assert!(!is_tool_allowed("tool1", Some(&filter)));
        assert!(is_tool_allowed("tool2", Some(&filter)));
        assert!(is_tool_allowed("tool3", Some(&filter)));
    }

    #[test]
    fn test_tool_filter_include_and_exclude() {
        use crate::routing::tool_filter::is_tool_allowed;
        let filter = ToolFilter {
            include: Some(vec![
                "tool1".to_string(),
                "tool2".to_string(),
                "tool3".to_string(),
            ]),
            exclude: Some(vec!["tool2".to_string()]),
        };

        assert!(is_tool_allowed("tool1", Some(&filter)));
        assert!(!is_tool_allowed("tool2", Some(&filter))); // excluded even though in include
        assert!(is_tool_allowed("tool3", Some(&filter)));
        assert!(!is_tool_allowed("tool4", Some(&filter))); // not in include list
    }

    #[test]
    fn test_tool_filter_no_filters() {
        use crate::routing::tool_filter::is_tool_allowed;
        let filter = ToolFilter {
            include: None,
            exclude: None,
        };

        assert!(is_tool_allowed("tool1", Some(&filter)));
        assert!(is_tool_allowed("tool2", Some(&filter)));
        assert!(is_tool_allowed("anything", Some(&filter)));
    }
}
