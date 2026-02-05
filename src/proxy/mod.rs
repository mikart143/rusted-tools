pub mod bridge;
pub mod client;
pub mod filter;
pub mod router;

// Re-export public API types (marked as allow unused for library consumers)
#[allow(unused_imports)]
pub use bridge::McpBridgeServer;
#[allow(unused_imports)]
pub use client::{McpClient, Tool, ToolCallRequest, ToolCallResponse};
#[allow(unused_imports)]
pub use filter::{apply_tool_filter, is_tool_allowed};
pub use router::Router;
