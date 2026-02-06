pub mod handlers;
pub(crate) mod mcp_sse_service;
pub mod routes;

use crate::config::AppConfig;
use crate::endpoint::EndpointManager;
use crate::routing::PathRouter;
use anyhow::Result;
use axum::Router;
use handlers::ApiState;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

pub async fn start_server(config: AppConfig) -> Result<()> {
    let addr = format!("{}:{}", config.http.host, config.http.port);

    // Initialize endpoint manager
    let manager = Arc::new(EndpointManager::new());
    manager.init_from_config(config.endpoints.clone()).await?;

    // Initialize router
    let router = Arc::new(PathRouter::new(manager.clone()));
    router.init_from_config(&config.endpoints)?;

    // Get routes before moving router into state
    let routes = router.list_routes();

    // Create app state
    let state = ApiState {
        manager: manager.clone(),
        router,
    };

    // Build the application
    let app = build_router(state).await?;

    // Create TCP listener
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("HTTP server listening on {}", addr);
    info!("Health check: http://{}/health", addr);
    info!("Server info: http://{}/info", addr);
    info!("Server list: http://{}/servers", addr);
    info!("");
    info!("MCP endpoints available at:");
    for (path, endpoint_name) in routes {
        info!(
            "  → http://{}/mcp/{} (endpoint: {})",
            addr, path, endpoint_name
        );
    }

    // Start the server
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(manager))
        .await?;

    Ok(())
}

async fn build_router(state: ApiState) -> Result<Router> {
    let ct = CancellationToken::new();

    // Start with base routes
    let mut app = Router::new()
        .merge(routes::health_routes())
        .merge(routes::management_routes())
        .merge(routes::mcp_routes());

    // Add MCP endpoints using polymorphic attach_http_route
    let routes = state.router.list_routes();
    for (path, endpoint_name) in routes {
        // Get endpoint instance
        let endpoint = match state.manager.get_endpoint(&endpoint_name) {
            Ok(endpoint) => endpoint,
            Err(e) => {
                tracing::warn!("Skipping endpoint for {}: {}", endpoint_name, e);
                continue;
            }
        };

        let endpoint_guard = endpoint.read().await;

        // Use polymorphic attach_http_route method
        // Note: attach_http_route takes ownership of the router
        let result = endpoint_guard
            .attach_http_route(app, &path, ct.child_token())
            .await;

        app = match result {
            Ok(router) => router,
            Err(e) => {
                tracing::error!(
                    "Failed to attach route for endpoint {}: {}. This is a fatal error.",
                    endpoint_name,
                    e
                );
                return Err(e.into());
            }
        };
    }

    // Add layers
    let app = app
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    Ok(app)
}

async fn shutdown_signal(manager: Arc<EndpointManager>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal, shutting down...");
        },
        _ = terminate => {
            info!("Received SIGTERM signal, shutting down...");
        },
    }

    // Gracefully shutdown all endpoints
    if let Err(e) = manager.shutdown().await {
        tracing::error!("Error during shutdown: {}", e);
    }
}
