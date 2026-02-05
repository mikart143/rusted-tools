pub mod local;
pub mod manager;
pub mod registry;
pub mod remote;

pub use manager::ServerManager;
// Re-export registry types (marked as allow unused for library consumers)
#[allow(unused_imports)]
pub use registry::{ServerInfo, ServerRegistry, ServerStatus, ServerType};
