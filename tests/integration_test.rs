use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use rusted_tools::api::handlers::ApiState;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

mod common;

/// Helper function to create a test app with state
async fn create_test_app() -> Router {
    let config = common::create_test_config();
    let manager = Arc::new(rusted_tools::endpoint::EndpointManager::new());

    // Initialize endpoints from config
    manager
        .init_from_config(config.endpoints.clone())
        .await
        .unwrap();

    let router = Arc::new(rusted_tools::routing::PathRouter::new(manager.clone()));
    router.init_from_config(&config.endpoints).unwrap();

    let state = ApiState { manager, router };

    Router::new()
        .merge(rusted_tools::api::routes::health_routes())
        .merge(rusted_tools::api::routes::management_routes())
        .merge(rusted_tools::api::routes::mcp_routes())
        .with_state(state)
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "rusted-tools");
}

#[tokio::test]
async fn test_info_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(Request::builder().uri("/info").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["name"], "rusted-tools");
    assert!(json["version"].is_string());
}

#[tokio::test]
async fn test_list_servers_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/servers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    let servers = json["servers"].as_array().unwrap();
    assert_eq!(servers.len(), 2);

    // Verify server names
    let names: Vec<&str> = servers
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"echo-server"));
    assert!(names.contains(&"remote-server"));
}

#[tokio::test]
async fn test_server_status_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/servers/echo-server/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["name"], "echo-server");
    assert_eq!(json["type"], "local");
    assert!(json["status"].is_string());
}

#[tokio::test]
async fn test_server_not_found() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/servers/nonexistent/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_start_server_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/servers/echo-server/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Either success or already running
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_stop_server_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/servers/echo-server/stop")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // The server might not be running, so either OK or NOT_RUNNING is acceptable
    assert!(
        response.status() == StatusCode::OK || response.status() == StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn test_mcp_list_tools_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/mcp/echo-server/tools")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["server"], "echo-server");
    assert!(json["tools"].is_array());
}

#[tokio::test]
async fn test_mcp_call_tool_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp/echo-server/tools/call")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["server"], "echo-server");
    assert_eq!(json["status"], "not_implemented");
}

#[tokio::test]
async fn test_mcp_sse_endpoint_not_implemented() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/mcp/echo-server/sse")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn test_mcp_messages_endpoint_not_implemented() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp/echo-server/messages")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn test_mcp_invalid_path() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/mcp/invalid-server/tools")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_concurrent_requests() {
    let app = create_test_app().await;

    // Create multiple concurrent requests
    let mut handles = vec![];

    for _ in 0..10 {
        let app = app.clone();
        let handle = tokio::spawn(async move {
            app.oneshot(
                Request::builder()
                    .uri("/servers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    for handle in handles {
        let response = handle.await.unwrap().unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
