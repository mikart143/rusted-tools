# CLAUDE.md

## Project Overview

Rusted-tools is a high-performance MCP (Model Context Protocol) proxy server written in Rust. It provides unified access to multiple MCP endpoints — both local (stdio-based) and remote (HTTP/SSE) — through a REST API. Acts as a bridge between clients (e.g. VS Code) and MCP tool servers.

## Commands

```bash
cargo build                              # Build the project
cargo run -- --config config.toml        # Run with config file
cargo test                               # Run all tests
cargo test --lib                         # Run unit tests only
cargo test --test integration_test       # Run integration tests only
cargo clippy                             # Lint
cargo fmt                                # Format code
```

CLI flags: `--config <path>`, `--log-level <trace|debug|info|warn|error>`, `--log-format <pretty|json>`

## Project Structure

```
src/
├── main.rs              # Entry point, CLI parsing (clap), logging init, shutdown signals
├── lib.rs               # Public module re-exports
├── error.rs             # ProxyError enum (thiserror), Result type alias, IntoResponse impl
├── config/
│   ├── mod.rs           # Config loading & validation (anyhow + config crate)
│   └── types.rs         # Serde-based config structs (TOML deserialization)
├── api/                 # HTTP layer (renamed from http/)
│   ├── mod.rs           # Axum server startup, router building, ApiState
│   ├── routes.rs        # Route group definitions (health, management, mcp)
│   ├── handlers.rs      # HTTP request handlers with Axum extractors
│   └── mcp_sse_service.rs  # SSE transport service factory
├── mcp/                 # MCP protocol concerns
│   ├── mod.rs           # Re-exports
│   ├── client.rs        # McpClient wrapper around rmcp RunningService
│   ├── types.rs         # ToolDefinition, ToolCallRequest, ToolCallResponse, ToolContent
│   └── bridge.rs        # StdioBridge: stdio <-> HTTP/SSE bridge (ServerHandler impl)
├── routing/             # Request routing (renamed from proxy/)
│   ├── mod.rs           # Re-exports
│   ├── path_router.rs   # PathRouter: path-to-endpoint routing (DashMap-based)
│   └── tool_filter.rs   # Tool include/exclude filtering
└── endpoint/            # Endpoint lifecycle (renamed from server/)
    ├── mod.rs           # EndpointKind enum dispatch
    ├── manager.rs       # EndpointManager lifecycle orchestration (DashMap)
    ├── registry.rs      # EndpointRegistry, EndpointInfo, EndpointStatus
    ├── local.rs         # LocalEndpoint (subprocess via TokioChildProcess)
    └── remote.rs        # RemoteEndpoint (HTTP reverse proxy)
tests/
├── integration_test.rs  # Full API endpoint integration tests
└── common/mod.rs        # Test utilities and helpers
```

## Architecture

- **Polymorphic endpoints**: `EndpointKind` enum wraps `LocalEndpoint` and `RemoteEndpoint`, with match-based dispatch for shared lifecycle methods
- **Concurrency**: `DashMap` for lock-free concurrent collections (EndpointManager, PathRouter); `Arc<RwLock<>>` for shared mutable state on individual resources
- **HTTP layer**: Axum 0.8 with `ApiState` shared via `State` extractor; CORS and tracing middleware
- **Graceful shutdown**: `CancellationToken` + `tokio::signal` (SIGTERM, SIGINT)
- **MCP transports**: stdio (local endpoints via rmcp TokioChildProcess) and HTTP/SSE (remote endpoints via StreamableHttpClientTransport + reverse proxy)

## Code Conventions

- **Error handling**: `ProxyError` enum with `thiserror` — each variant maps to an HTTP status code. `anyhow` used only in config loading. Custom `Result<T>` type alias in `error.rs`.
- **Async**: tokio runtime, `#[tokio::test]` for async tests
- **Serialization**: serde derives on all config/API types. `#[serde(tag = "type", rename_all = "lowercase")]` for tagged enum variants. TOML for config files.
- **Logging**: `tracing` crate (`info!`, `debug!`, `warn!`, `error!`). Configured via `tracing-subscriber` with env-filter, supports JSON and pretty formats.
- **Module pattern**: `mod.rs` files re-export public items. `lib.rs` exposes top-level modules. `#[allow(unused_imports)]` on conditional re-exports.
- **Naming**: PascalCase types, snake_case functions/modules, UPPER_CASE constants
- **Testing**: `#[cfg(test)] mod tests` inline in each module. Integration tests use `axum::body::Body` + `tower::ServiceExt::oneshot`. Dev deps: tempfile.
- **No rustfmt.toml or clippy.toml** — uses Rust 2021 edition defaults

## Key Types

| Type | Module | Purpose |
|------|--------|---------|
| `AppConfig` | config | Top-level config (TOML root) |
| `HttpConfig` | config | HTTP server settings (`[http]`) |
| `McpConfig` | config | MCP settings (`[mcp]`): `request_timeout_secs` (default: 30s, min: 5s), `restart_delay_ms` (default: 500ms) |
| `EndpointConfig` | config | Single endpoint config (`[[endpoints]]`) |
| `EndpointKindConfig` | config | Local vs Remote discriminator |
| `EndpointKind` | endpoint | Runtime enum: LocalEndpoint / RemoteEndpoint |
| `EndpointManager` | endpoint | Lifecycle orchestration (start/stop/restart) |
| `EndpointRegistry` | endpoint | Metadata registry (DashMap-based) |
| `McpClient` | mcp | rmcp RunningService wrapper with pagination support |
| `StdioBridge` | mcp | stdio-to-HTTP/SSE ServerHandler bridge (ServerHandler impl) |
| `ToolDefinition` | mcp | MCP tool metadata |
| `PathRouter` | routing | URL path to endpoint routing |
| `ApiState` | api | Shared Axum handler state with mcp_request_timeout |

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| rmcp 0.14 | MCP SDK (protocol, transports, client/server) |
| axum 0.8 | HTTP framework |
| tokio 1.49 | Async runtime |
| thiserror / anyhow | Error handling |
| dashmap 6 | Concurrent hash maps |
| tracing | Structured logging |
| serde / serde_json / toml | Serialization |
| clap 4 | CLI argument parsing |
| tower-http | HTTP middleware (CORS, tracing) |
| axum-reverse-proxy | Remote endpoint reverse proxying |

## Configuration

TOML format with sections: `[http]`, `[logging]`, `[mcp]`, `[[endpoints]]`. Validated at load time:
- Unique endpoint names and valid path characters
- Valid log levels (trace, debug, info, warn, error) and formats (pretty, json)
- MCP `request_timeout_secs` >= 5 seconds (default: 30 seconds)

Example `[mcp]` section:
```toml
[mcp]
request_timeout_secs = 30
```

See `config.toml` and `examples/*.toml` for reference configurations.
