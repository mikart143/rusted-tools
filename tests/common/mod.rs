use rusted_tools::{
    api::handlers::ApiState,
    config::{AppConfig, EndpointConfig, EndpointKindConfig, HttpConfig},
    endpoint::EndpointManager,
    routing::PathRouter,
};
use std::{collections::HashMap, sync::Arc};

/// Create a standard test configuration with echo and remote servers
pub fn create_test_config() -> AppConfig {
    AppConfig {
        http: HttpConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
        },
        logging: Default::default(),
        endpoints: vec![
            EndpointConfig {
                name: "echo-server".to_string(),
                endpoint_type: EndpointKindConfig::Local {
                    command: "echo".to_string(),
                    args: vec!["hello world".to_string()],
                    env: HashMap::new(),
                    auto_start: true,
                    restart_on_failure: false,
                },
                tools: None,
                path: Some("echo-server".to_string()),
            },
            EndpointConfig {
                name: "remote-server".to_string(),
                endpoint_type: EndpointKindConfig::Remote {
                    url: "http://localhost:9000".to_string(),
                },
                tools: None,
                path: Some("remote-server".to_string()),
            },
        ],
    }
}

/// Create test configuration for handler unit tests
pub async fn create_test_state() -> ApiState {
    let manager = Arc::new(EndpointManager::new());

    // Create configs
    let configs = vec![
        EndpointConfig {
            name: "test-local".to_string(),
            endpoint_type: EndpointKindConfig::Local {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                env: HashMap::new(),
                auto_start: true,
                restart_on_failure: false,
            },
            tools: None,
            path: Some("test-local".to_string()),
        },
        EndpointConfig {
            name: "test-remote".to_string(),
            endpoint_type: EndpointKindConfig::Remote {
                url: "http://localhost:8080".to_string(),
            },
            tools: None,
            path: Some("test-remote".to_string()),
        },
    ];

    // Initialize manager from configs
    manager.init_from_config(configs.clone()).await.unwrap();

    let router = Arc::new(PathRouter::new(manager.clone()));
    router.init_from_config(&configs).unwrap();

    ApiState { manager, router }
}
