use axum::Router;
use rusted_tools::{
    api::handlers::ApiState,
    config::{AppConfig, EndpointConfig, EndpointKindConfig, HttpConfig, McpConfig},
    endpoint::EndpointManager,
    routing::PathRouter,
};
use std::{collections::HashMap, sync::Arc, time::Duration};

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
        mcp: McpConfig::default(),
        endpoints: vec![
            EndpointConfig {
                name: "local-stub".to_string(),
                endpoint_type: EndpointKindConfig::Local {
                    command: "cat".to_string(),
                    args: vec![],
                    env: HashMap::new(),
                    auto_start: false,
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
        mcp: McpConfig::default(),
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
        mcp: McpConfig::default(),
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
        mcp: McpConfig::default(),
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
    let manager = Arc::new(EndpointManager::new_with_restart_delay(
        Duration::from_millis(config.mcp.restart_delay_ms),
    ));
    manager
        .init_from_config(config.endpoints.clone())
        .await
        .unwrap();

    let router = Arc::new(PathRouter::new(manager.clone()));

    let state = ApiState {
        manager,
        router,
        mcp_request_timeout: Duration::from_secs(config.mcp.request_timeout_secs),
    };

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
