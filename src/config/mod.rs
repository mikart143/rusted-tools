pub mod types;

use anyhow::{Context, Result};
use config::{Config, File};
use std::path::Path;
pub use types::*;

/// Load configuration from a TOML file
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<ProxyConfig> {
    let path = path.as_ref();

    let config = Config::builder()
        .add_source(File::from(path))
        .build()
        .with_context(|| format!("Failed to load config from: {}", path.display()))?;

    let proxy_config: ProxyConfig = config
        .try_deserialize()
        .context("Failed to deserialize configuration")?;

    validate_config(&proxy_config)?;

    Ok(proxy_config)
}

/// Validate the loaded configuration
fn validate_config(config: &ProxyConfig) -> Result<()> {
    // Validate that server paths are unique
    let mut paths = std::collections::HashSet::new();
    for server in &config.mcp_servers {
        let path = server.get_path();
        if !paths.insert(path.clone()) {
            anyhow::bail!("Duplicate server path '{}' found in configuration", path);
        }
    }

    // Validate that server names are unique
    let mut names = std::collections::HashSet::new();
    for server in &config.mcp_servers {
        if !names.insert(server.name.clone()) {
            anyhow::bail!(
                "Duplicate server name '{}' found in configuration",
                server.name
            );
        }
    }

    // Validate server paths don't contain special characters
    for server in &config.mcp_servers {
        let path = server.get_path();
        if path.contains('/') || path.contains('\\') || path.contains('.') {
            anyhow::bail!(
                "Server path '{}' contains invalid characters (/, \\, or .)",
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
[server]
host = "0.0.0.0"
port = 8080

[logging]
level = "debug"
format = "json"

[[mcp_servers]]
name = "test-server"
type = "local"
command = "echo"
args = ["hello"]
path = "test"
"#;

        let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let config = load_config(temp_file.path()).unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.format, "json");
        assert_eq!(config.mcp_servers.len(), 1);
        assert_eq!(config.mcp_servers[0].name, "test-server");
    }

    #[test]
    fn test_load_config_with_defaults() {
        let config_content = r#"
[server]

[logging]

[[mcp_servers]]
name = "test-server"
type = "local"
command = "echo"
args = ["hello"]
"#;

        let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let config = load_config(temp_file.path()).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.logging.format, "pretty");
    }

    #[test]
    fn test_validate_duplicate_paths() {
        let config = ProxyConfig {
            server: ServerConfig::default(),
            logging: LoggingConfig::default(),
            mcp_servers: vec![
                McpServerConfig {
                    name: "server1".to_string(),
                    server_type: McpServerType::Local {
                        command: "echo".to_string(),
                        args: vec![],
                        env: Default::default(),
                        auto_start: true,
                        restart_on_failure: false,
                    },
                    tools: None,
                    path: Some("test".to_string()),
                },
                McpServerConfig {
                    name: "server2".to_string(),
                    server_type: McpServerType::Local {
                        command: "echo".to_string(),
                        args: vec![],
                        env: Default::default(),
                        auto_start: true,
                        restart_on_failure: false,
                    },
                    tools: None,
                    path: Some("test".to_string()),
                },
            ],
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_path_characters() {
        let config = ProxyConfig {
            server: ServerConfig::default(),
            logging: LoggingConfig::default(),
            mcp_servers: vec![McpServerConfig {
                name: "server1".to_string(),
                server_type: McpServerType::Local {
                    command: "echo".to_string(),
                    args: vec![],
                    env: Default::default(),
                    auto_start: true,
                    restart_on_failure: false,
                },
                tools: None,
                path: Some("test/path".to_string()),
            }],
        };

        assert!(validate_config(&config).is_err());
    }
}
