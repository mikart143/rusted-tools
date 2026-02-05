use rusted_tools::{
    config::{McpServerConfig, McpServerType, ProxyConfig, ServerConfig},
    http::handlers::AppState,
    proxy::Router as ProxyRouter,
    server::ServerManager,
};
use std::{collections::HashMap, sync::Arc};

/// Create a standard test configuration with echo and remote servers
pub fn create_test_config() -> ProxyConfig {
    ProxyConfig {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
        },
        logging: Default::default(),
        mcp_servers: vec![
            McpServerConfig {
                name: "echo-server".to_string(),
                server_type: McpServerType::Local {
                    command: "echo".to_string(),
                    args: vec!["hello world".to_string()],
                    env: HashMap::new(),
                    auto_start: true,
                    restart_on_failure: false,
                },
                tools: None,
                path: Some("echo-server".to_string()),
            },
            McpServerConfig {
                name: "remote-server".to_string(),
                server_type: McpServerType::Remote {
                    url: "http://localhost:9000".to_string(),
                },
                tools: None,
                path: Some("remote-server".to_string()),
            },
        ],
    }
}

/// Create test configuration for handler unit tests
pub async fn create_test_state() -> AppState {
    let manager = Arc::new(ServerManager::new());
    
    // Create configs
    let configs = vec![
        McpServerConfig {
            name: "test-local".to_string(),
            server_type: McpServerType::Local {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                env: HashMap::new(),
                auto_start: true,
                restart_on_failure: false,
            },
            tools: None,
            path: Some("test-local".to_string()),
        },
        McpServerConfig {
            name: "test-remote".to_string(),
            server_type: McpServerType::Remote {
                url: "http://localhost:8080".to_string(),
            },
            tools: None,
            path: Some("test-remote".to_string()),
        },
    ];
    
    // Initialize manager from configs
    manager.init_from_config(configs.clone()).await.unwrap();

    let router = Arc::new(ProxyRouter::new(manager.clone()));
    router.init_from_config(&configs).unwrap();
    
    AppState { manager, router }
}
