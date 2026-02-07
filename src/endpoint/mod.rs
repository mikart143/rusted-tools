pub(crate) mod client_holder;
pub(crate) mod local;
pub(crate) mod manager;
pub(crate) mod registry;
pub(crate) mod remote;

pub(crate) use local::LocalEndpoint;
pub use manager::EndpointManager;
pub(crate) use remote::RemoteEndpoint;

use crate::error::Result;
use crate::mcp::McpClient;
use axum::Router;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Enum wrapper for polymorphic endpoint handling
/// This allows us to store different endpoint types in the same collection
#[derive(Clone)]
pub(crate) enum EndpointKind {
    Local(LocalEndpoint),
    Remote(RemoteEndpoint),
}

pub(crate) trait HttpTransportAdapter {
    fn attach_http_route<S>(
        &self,
        router: Router<S>,
        path: &str,
        ct: CancellationToken,
    ) -> Result<Router<S>>
    where
        S: Clone + Send + Sync + 'static;
}

impl EndpointKind {
    pub(crate) async fn start(&mut self) -> Result<()> {
        match self {
            EndpointKind::Local(s) => s.start().await,
            EndpointKind::Remote(s) => s.start().await,
        }
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        match self {
            EndpointKind::Local(s) => s.stop().await,
            EndpointKind::Remote(s) => s.stop().await,
        }
    }

    pub(crate) async fn get_or_create_client(&self) -> Result<Arc<McpClient>> {
        match self {
            EndpointKind::Local(s) => s.get_or_create_client().await,
            EndpointKind::Remote(s) => s.get_or_create_client().await,
        }
    }
}

impl HttpTransportAdapter for EndpointKind {
    fn attach_http_route<S>(
        &self,
        router: Router<S>,
        path: &str,
        ct: CancellationToken,
    ) -> Result<Router<S>>
    where
        S: Clone + Send + Sync + 'static,
    {
        match self {
            EndpointKind::Local(s) => HttpTransportAdapter::attach_http_route(s, router, path, ct),
            EndpointKind::Remote(s) => HttpTransportAdapter::attach_http_route(s, router, path, ct),
        }
    }
}
