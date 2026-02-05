pub mod handlers;
pub mod mcp_sse_service;
pub mod routes;

use crate::config::ProxyConfig;
use crate::proxy::Router as ProxyRouter;
use crate::server::ServerManager;
use anyhow::Result;
use axum::Router;
use handlers::AppState;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

pub async fn start_server(config: ProxyConfig) -> Result<()> {
    let addr = format!("{}:{}", config.server.host, config.server.port);
    
    // Initialize server manager
    let manager = Arc::new(ServerManager::new());
    manager.init_from_config(config.mcp_servers.clone()).await?;
    
    // Initialize router
    let router = Arc::new(ProxyRouter::new(manager.clone()));
    router.init_from_config(&config.mcp_servers)?;
    
    // Get routes before moving router into state
    let routes = router.list_routes();
    
    // Create app state
    let state = AppState {
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
    for (path, server_name) in routes {
        info!("  → http://{}/mcp/{} (server: {})", addr, path, server_name);
    }

    // Start the server
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(manager))
        .await?;

    Ok(())
}

async fn build_router(state: AppState) -> Result<Router> {
    use tokio_util::sync::CancellationToken;
    use axum_reverse_proxy::ReverseProxy;
    use crate::server::registry::ServerType;
    
    let ct = CancellationToken::new();
    
    // Start with base routes
    let mut app = Router::new()
        .merge(routes::health_routes())
        .merge(routes::management_routes())
        .merge(routes::mcp_routes());
    
    // Add MCP endpoints based on server type
    let routes = state.router.list_routes();
    for (path, server_name) in routes {
        // Get server info to determine type
        let server_info = match state.manager.get_server_info(&server_name) {
            Ok(info) => info,
            Err(e) => {
                tracing::warn!("Skipping endpoint for {}: {}", server_name, e);
                continue;
            }
        };
        
        match server_info.server_type {
            ServerType::Local => {
                // Local servers: Use McpBridgeServer with SSE (stdio → SSE bridge)
                info!("Setting up SSE bridge for local server {} at /mcp/{}", server_name, path);
                
                let client = match state.manager.get_mcp_client(&server_name).await {
                    Ok(client) => client,
                    Err(e) => {
                        tracing::warn!("Skipping SSE endpoint for local server {}: {}", server_name, e);
                        continue;
                    }
                };
                
                let sse_service = mcp_sse_service::create_mcp_sse_service(
                    client,
                    server_name.clone(),
                    ct.child_token(),
                );
                
                // Nest the SSE service at /mcp/{path}
                app = app.nest_service(&format!("/mcp/{}", path), sse_service);
            }
            ServerType::Remote => {
                // Remote servers: Use direct HTTP/SSE reverse proxy (no protocol translation)
                let remote_server = match state.manager.get_remote_server(&server_name) {
                    Ok(server) => server,
                    Err(e) => {
                        tracing::warn!("Skipping endpoint for remote server {}: {}", server_name, e);
                        continue;
                    }
                };
                
                info!(
                    "Setting up HTTP reverse proxy for remote server {} at /mcp/{} → {}",
                    server_name, path, remote_server.url
                );
                
                // Create reverse proxy that forwards all requests to remote server
                let proxy = ReverseProxy::new(
                    &format!("/mcp/{}", path),
                    &remote_server.url
                );
                
                // Merge the proxy router into the main app
                app = app.merge(proxy);
            }
        }
    }
    
    // Add layers
    let app = app
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    Ok(app)
}

async fn shutdown_signal(manager: Arc<ServerManager>) {
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

    // Gracefully shutdown all servers
    if let Err(e) = manager.shutdown().await {
        tracing::error!("Error during shutdown: {}", e);
    }
}
