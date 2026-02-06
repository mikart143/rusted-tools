pub mod local;
pub mod manager;
pub mod registry;
pub mod remote;
pub mod traits;

pub use local::LocalEndpoint;
pub use manager::EndpointManager;
#[allow(unused_imports)]
pub use registry::{EndpointInfo, EndpointRegistry, EndpointStatus, EndpointType};
pub use remote::RemoteEndpoint;
pub use traits::EndpointInstance;

use crate::error::Result;
use crate::mcp::McpClient;
use async_trait::async_trait;
use axum::Router;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Enum wrapper for polymorphic endpoint handling
/// This allows us to store different endpoint types in the same collection
#[derive(Clone)]
pub enum EndpointKind {
    Local(LocalEndpoint),
    Remote(RemoteEndpoint),
}

#[async_trait]
impl EndpointInstance for EndpointKind {
    fn name(&self) -> &str {
        match self {
            EndpointKind::Local(s) => s.name(),
            EndpointKind::Remote(s) => s.name(),
        }
    }

    fn path(&self) -> &str {
        match self {
            EndpointKind::Local(s) => s.path(),
            EndpointKind::Remote(s) => s.path(),
        }
    }

    fn endpoint_type(&self) -> EndpointType {
        match self {
            EndpointKind::Local(s) => s.endpoint_type(),
            EndpointKind::Remote(s) => s.endpoint_type(),
        }
    }

    async fn start(&mut self) -> Result<()> {
        match self {
            EndpointKind::Local(s) => s.start().await,
            EndpointKind::Remote(s) => s.start().await,
        }
    }

    async fn stop(&mut self) -> Result<()> {
        match self {
            EndpointKind::Local(s) => s.stop().await,
            EndpointKind::Remote(s) => s.stop().await,
        }
    }

    async fn get_or_create_client(&self) -> Result<Arc<McpClient>> {
        match self {
            EndpointKind::Local(s) => s.get_or_create_client().await,
            EndpointKind::Remote(s) => s.get_or_create_client().await,
        }
    }

    fn is_started(&self) -> bool {
        match self {
            EndpointKind::Local(s) => s.is_started(),
            EndpointKind::Remote(s) => s.is_started(),
        }
    }

    async fn attach_http_route<S>(
        &self,
        router: Router<S>,
        path: &str,
        ct: CancellationToken,
    ) -> Result<Router<S>>
    where
        S: Clone + Send + Sync + 'static,
    {
        match self {
            EndpointKind::Local(s) => s.attach_http_route(router, path, ct).await,
            EndpointKind::Remote(s) => s.attach_http_route(router, path, ct).await,
        }
    }
}
