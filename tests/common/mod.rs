use axum::Router;
use rusted_tools::{
    api::handlers::ApiState,
    config::{AppConfig, EndpointConfig, EndpointKindConfig, HttpConfig},
    endpoint::EndpointManager,
    routing::PathRouter,
};
use std::{collections::HashMap, sync::Arc};

// ──────────────────────────────────────────────
// Tier 1: Offline configs (no real MCP servers)
// ──────────────────────────────────────────────

/// Config with endpoints registered but NOT auto-started.
/// Safe for testing API routing, listing, status, error paths.
pub fn create_offline_config() -> AppConfig {
    AppConfig {
        http: HttpConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
        },
        logging: Default::default(),
        endpoints: vec![
            EndpointConfig {
                name: "local-stub".to_string(),
                endpoint_type: EndpointKindConfig::Local {
                    command: "cat".to_string(),
                    args: vec![],
                    env: HashMap::new(),
                    auto_start: false,
                    restart_on_failure: false,
                },
                tools: None,
            },
            EndpointConfig {
                name: "remote-stub".to_string(),
                endpoint_type: EndpointKindConfig::Remote {
                    url: "http://127.0.0.1:19876".to_string(),
                },
                tools: None,
            },
        ],
    }
}

// ──────────────────────────────────────────────
// Tier 2: Live configs (real MCP servers)
// ──────────────────────────────────────────────

/// Config with real remote MCP endpoint (Microsoft Learn).
pub fn create_live_remote_config() -> AppConfig {
    AppConfig {
        http: HttpConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
        },
        logging: Default::default(),
        endpoints: vec![EndpointConfig {
            name: "microsoft-learn".to_string(),
            endpoint_type: EndpointKindConfig::Remote {
                url: "https://learn.microsoft.com/api/mcp".to_string(),
            },
            tools: None,
        }],
    }
}

/// Config with real local MCP endpoint (Docker mcp/time).
pub fn create_live_local_config() -> AppConfig {
    AppConfig {
        http: HttpConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
        },
        logging: Default::default(),
        endpoints: vec![EndpointConfig {
            name: "time".to_string(),
            endpoint_type: EndpointKindConfig::Local {
                command: "docker".to_string(),
                args: vec![
                    "run".to_string(),
                    "--rm".to_string(),
                    "-i".to_string(),
                    "mcp/time".to_string(),
                ],
                env: HashMap::new(),
                auto_start: false,
                restart_on_failure: false,
            },
            tools: None,
        }],
    }
}

/// Config combining both live endpoints.
pub fn create_live_full_config() -> AppConfig {
    AppConfig {
        http: HttpConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
        },
        logging: Default::default(),
        endpoints: vec![
            EndpointConfig {
                name: "microsoft-learn".to_string(),
                endpoint_type: EndpointKindConfig::Remote {
                    url: "https://learn.microsoft.com/api/mcp".to_string(),
                },
                tools: None,
            },
            EndpointConfig {
                name: "time".to_string(),
                endpoint_type: EndpointKindConfig::Local {
                    command: "docker".to_string(),
                    args: vec![
                        "run".to_string(),
                        "--rm".to_string(),
                        "-i".to_string(),
                        "mcp/time".to_string(),
                    ],
                    env: HashMap::new(),
                    auto_start: false,
                    restart_on_failure: false,
                },
                tools: None,
            },
        ],
    }
}

// ──────────────────────────────────────────────
// Shared helpers
// ──────────────────────────────────────────────

/// Build a test Router from the given config (no HTTP server, uses tower::oneshot).
pub async fn build_test_app(config: &AppConfig) -> Router {
    let manager = Arc::new(EndpointManager::new());
    manager
        .init_from_config(config.endpoints.clone())
        .await
        .unwrap();

    let router = Arc::new(PathRouter::new(manager.clone()));
    router.init_from_config(&config.endpoints).unwrap();

    let state = ApiState { manager, router };

    Router::new()
        .merge(rusted_tools::api::routes::health_routes())
        .merge(rusted_tools::api::routes::management_routes())
        .merge(rusted_tools::api::routes::mcp_routes())
        .with_state(state)
}

/// Helper to extract JSON from a response body.
pub async fn response_json(response: axum::http::Response<axum::body::Body>) -> serde_json::Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}
