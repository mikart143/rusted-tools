use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub http: HttpConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
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
pub struct EndpointConfig {
    pub name: String,
    #[serde(flatten)]
    pub endpoint_type: EndpointKindConfig,
    #[serde(default)]
    pub tools: Option<ToolFilter>,
    pub path: Option<String>, // URL path, defaults to name if not set
}

impl EndpointConfig {
    /// Get the URL path for this endpoint (defaults to name if not specified)
    pub fn get_path(&self) -> String {
        self.path.clone().unwrap_or_else(|| self.name.clone())
    }

    /// Extract local endpoint settings from this config
    /// Panics if this is not a local endpoint config (should check type first)
    pub fn to_local_settings(&self) -> LocalEndpointSettings {
        match &self.endpoint_type {
            EndpointKindConfig::Local {
                command,
                args,
                env,
                restart_on_failure,
                ..
            } => LocalEndpointSettings {
                command: command.clone(),
                args: args.clone(),
                env: env.clone(),
                path: self.get_path(),
                restart_on_failure: *restart_on_failure,
            },
            _ => panic!("Expected local endpoint configuration"),
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
        #[serde(default)]
        restart_on_failure: bool,
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

/// Local endpoint settings extracted from config
#[derive(Debug, Clone)]
pub struct LocalEndpointSettings {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub path: String,
    pub restart_on_failure: bool,
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
