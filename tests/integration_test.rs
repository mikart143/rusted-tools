use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

mod common;

// ============================================================================
// TIER 1: OFFLINE INTEGRATION TESTS
// No Docker, no network. Run by default with `cargo test`.
// ============================================================================

mod offline {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

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
        let json = common::response_json(response).await;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["service"], "rusted-tools");
    }

    #[tokio::test]
    async fn test_info_endpoint() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

        let response = app
            .oneshot(Request::builder().uri("/info").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = common::response_json(response).await;
        assert_eq!(json["name"], "rusted-tools");
        assert!(json["version"].is_string());
    }

    #[tokio::test]
    async fn test_list_servers_returns_registered_endpoints() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

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
        let json = common::response_json(response).await;
        let servers = json["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 2);

        let names: Vec<&str> = servers
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"local-stub"));
        assert!(names.contains(&"remote-stub"));
    }

    #[tokio::test]
    async fn test_server_status_local() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/servers/local-stub/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = common::response_json(response).await;
        assert_eq!(json["name"], "local-stub");
        assert_eq!(json["type"], "local");
        assert_eq!(json["status"], "stopped");
    }

    #[tokio::test]
    async fn test_server_status_remote() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/servers/remote-stub/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = common::response_json(response).await;
        assert_eq!(json["name"], "remote-stub");
        assert_eq!(json["type"], "remote");
        assert_eq!(json["status"], "stopped");
    }

    #[tokio::test]
    async fn test_server_not_found_returns_404() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

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
    async fn test_stop_stopped_server_returns_error() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/local-stub/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_mcp_invalid_path_returns_404() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/mcp/nonexistent/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_mcp_tools_on_stopped_endpoint_returns_error() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/mcp/local-stub/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Endpoint not running -> ServerNotRunning (503) or McpProtocol (502)
        assert!(
            response.status() == StatusCode::SERVICE_UNAVAILABLE
                || response.status() == StatusCode::BAD_GATEWAY
        );
    }

    #[tokio::test]
    async fn test_mcp_call_tool_with_empty_body_returns_error() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp/local-stub/tools/call")
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Empty body cannot be parsed as JSON -> 400 or 422
        assert!(
            response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let config = common::create_offline_config();
        let app = common::build_test_app(&config).await;

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

        for handle in handles {
            let response = handle.await.unwrap().unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
    }
}

// ============================================================================
// TIER 2: LIVE INTEGRATION TESTS
// Require Docker and/or network. Run with: cargo test -- --ignored
// ============================================================================

mod live {
    use super::*;

    // --- Remote MCP: Microsoft Learn ---

    #[tokio::test]
    #[ignore = "requires network access to learn.microsoft.com"]
    async fn test_remote_microsoft_learn_list_tools() {
        let config = common::create_live_remote_config();
        let app = common::build_test_app(&config).await;

        // Start the remote endpoint via HTTP API
        let start_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/microsoft-learn/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            start_response.status(),
            StatusCode::OK,
            "Failed to start microsoft-learn endpoint"
        );

        // List tools
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/mcp/microsoft-learn/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = common::response_json(response).await;
        assert_eq!(json["server"], "microsoft-learn");

        let tools = json["tools"].as_array().unwrap();
        assert!(
            !tools.is_empty(),
            "Microsoft Learn MCP should expose at least one tool"
        );

        // Verify tool shape
        let first_tool = &tools[0];
        assert!(first_tool["name"].is_string());
        assert!(first_tool["input_schema"].is_object());
    }

    #[tokio::test]
    #[ignore = "requires network access to learn.microsoft.com"]
    async fn test_remote_microsoft_learn_start_stop_lifecycle() {
        let config = common::create_live_remote_config();
        let app = common::build_test_app(&config).await;

        // Start
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/microsoft-learn/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify running
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/servers/microsoft-learn/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = common::response_json(response).await;
        assert_eq!(json["status"], "running");

        // Stop
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/microsoft-learn/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify stopped
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/servers/microsoft-learn/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = common::response_json(response).await;
        assert_eq!(json["status"], "stopped");
    }

    // --- Local MCP: Docker mcp/time ---

    #[tokio::test]
    #[ignore = "requires Docker with mcp/time image"]
    async fn test_local_docker_time_list_tools() {
        let config = common::create_live_local_config();
        let app = common::build_test_app(&config).await;

        // Start the local endpoint
        let start_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/time/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            start_response.status(),
            StatusCode::OK,
            "Failed to start time endpoint (is Docker running with mcp/time image?)"
        );

        // List tools
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/mcp/time/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = common::response_json(response).await;
        assert_eq!(json["server"], "time");

        let tools = json["tools"].as_array().unwrap();
        assert!(
            !tools.is_empty(),
            "mcp/time should expose at least one tool"
        );

        // Cleanup
        let _ = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/time/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
    }

    #[tokio::test]
    #[ignore = "requires Docker with mcp/time image"]
    async fn test_local_docker_time_call_tool() {
        let config = common::create_live_local_config();
        let app = common::build_test_app(&config).await;

        // Start
        let start_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/time/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(start_response.status(), StatusCode::OK);

        // List tools to find a valid tool name
        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/mcp/time/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_response.status(), StatusCode::OK);

        let list_json = common::response_json(list_response).await;
        let tools = list_json["tools"].as_array().unwrap();
        assert!(!tools.is_empty());

        let tool_name = tools[0]["name"].as_str().unwrap();

        // Call the tool
        let call_body = serde_json::json!({
            "name": tool_name,
            "arguments": {}
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp/time/tools/call")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&call_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = common::response_json(response).await;
        assert!(
            json["content"].is_array(),
            "Tool call response should have content array"
        );

        // Cleanup
        let _ = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/time/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
    }

    #[tokio::test]
    #[ignore = "requires Docker with mcp/time image"]
    async fn test_local_docker_time_start_stop_lifecycle() {
        let config = common::create_live_local_config();
        let app = common::build_test_app(&config).await;

        // Initial state: stopped
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/servers/time/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = common::response_json(response).await;
        assert_eq!(json["status"], "stopped");

        // Start
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/time/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify running
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/servers/time/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = common::response_json(response).await;
        assert_eq!(json["status"], "running");

        // Double-start should return Conflict
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/time/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);

        // Stop
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/time/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify stopped
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/servers/time/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = common::response_json(response).await;
        assert_eq!(json["status"], "stopped");

        // Double-stop should return error
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/servers/time/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    // --- Full stack: both endpoints ---

    #[tokio::test]
    #[ignore = "requires Docker and network"]
    async fn test_full_stack_list_all_servers() {
        let config = common::create_live_full_config();
        let app = common::build_test_app(&config).await;

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
        let json = common::response_json(response).await;
        let servers = json["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 2);

        let names: Vec<&str> = servers
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"microsoft-learn"));
        assert!(names.contains(&"time"));
    }
}
