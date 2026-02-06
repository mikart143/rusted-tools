pub mod types;

use anyhow::{Context, Result};
use config::{Config, File};
use std::path::Path;
pub use types::*;

/// Load configuration from a TOML file
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<AppConfig> {
    let path = path.as_ref();

    let config = Config::builder()
        .add_source(File::from(path))
        .build()
        .with_context(|| format!("Failed to load config from: {}", path.display()))?;

    let app_config: AppConfig = config
        .try_deserialize()
        .context("Failed to deserialize configuration")?;

    validate_config(&app_config)?;

    Ok(app_config)
}

/// Validate the loaded configuration
fn validate_config(config: &AppConfig) -> Result<()> {
    // Validate that endpoint names/paths are unique
    let mut names = std::collections::HashSet::new();
    for endpoint in &config.endpoints {
        if !names.insert(endpoint.name.clone()) {
            anyhow::bail!(
                "Duplicate endpoint name '{}' found in configuration",
                endpoint.name
            );
        }
    }

    // Validate endpoint paths don't contain special characters
    for endpoint in &config.endpoints {
        let path = endpoint.name.clone();
        if path.contains('/') || path.contains('\\') || path.contains('.') {
            anyhow::bail!(
                "Endpoint path '{}' contains invalid characters (/, \\, or .)",
                path
            );
        }
    }

    // Validate log level
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    if !valid_levels.contains(&config.logging.level.as_str()) {
        anyhow::bail!(
            "Invalid log level '{}'. Valid levels: {}",
            config.logging.level,
            valid_levels.join(", ")
        );
    }

    // Validate log format
    let valid_formats = ["pretty", "json"];
    if !valid_formats.contains(&config.logging.format.as_str()) {
        anyhow::bail!(
            "Invalid log format '{}'. Valid formats: {}",
            config.logging.format,
            valid_formats.join(", ")
        );
    }

    // Validate MCP request timeout
    if config.mcp.request_timeout_secs < 5 {
        anyhow::bail!(
            "Invalid mcp.request_timeout_secs: {}. Minimum value is 5 seconds",
            config.mcp.request_timeout_secs
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_valid_config() {
        let config_content = r#"
[http]
host = "0.0.0.0"
port = 8080

[logging]
level = "debug"
format = "json"

[[endpoints]]
name = "test-server"
type = "local"
command = "echo"
args = ["hello"]
"#;

        let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let config = load_config(temp_file.path()).unwrap();
        assert_eq!(config.http.host, "0.0.0.0");
        assert_eq!(config.http.port, 8080);
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.format, "json");
        assert_eq!(config.endpoints.len(), 1);
        assert_eq!(config.endpoints[0].name, "test-server");
    }

    #[test]
    fn test_load_config_with_defaults() {
        let config_content = r#"
[http]

[logging]

[[endpoints]]
name = "test-server"
type = "local"
command = "echo"
args = ["hello"]
"#;

        let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let config = load_config(temp_file.path()).unwrap();
        assert_eq!(config.http.host, "127.0.0.1");
        assert_eq!(config.http.port, 3000);
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.logging.format, "pretty");
    }

    #[test]
    fn test_validate_duplicate_paths() {
        let config = AppConfig {
            http: HttpConfig::default(),
            logging: LoggingConfig::default(),
            mcp: Default::default(),
            endpoints: vec![
                EndpointConfig {
                    name: "server".to_string(),
                    endpoint_type: EndpointKindConfig::Local {
                        command: "echo".to_string(),
                        args: vec![],
                        env: Default::default(),
                        auto_start: true,
                    },
                    tools: None,
                },
                EndpointConfig {
                    name: "server".to_string(),
                    endpoint_type: EndpointKindConfig::Local {
                        command: "echo".to_string(),
                        args: vec![],
                        env: Default::default(),
                        auto_start: true,
                    },
                    tools: None,
                },
            ],
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_path_characters() {
        let config = AppConfig {
            http: HttpConfig::default(),
            logging: LoggingConfig::default(),
            mcp: Default::default(),
            endpoints: vec![EndpointConfig {
                name: "server/path".to_string(),
                endpoint_type: EndpointKindConfig::Local {
                    command: "echo".to_string(),
                    args: vec![],
                    env: Default::default(),
                    auto_start: true,
                },
                tools: None,
            }],
        };

        assert!(validate_config(&config).is_err());
    }
}
