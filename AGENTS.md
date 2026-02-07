# AGENTS.md

## Project Overview

Rusted-tools is a high-performance MCP (Model Context Protocol) proxy server written in Rust. It provides unified access to multiple MCP endpoints — both local (stdio-based) and remote (HTTP/SSE) — through a REST API. Acts as a bridge between AI agents/clients (e.g. VS Code, AI frameworks) and MCP tool servers.

## Quick Start

```bash
cargo build                              # Build the project
cargo run -- --config config.toml        # Run with config file
cargo test                               # Run all tests
```

CLI flags: `--config <path>`, `--log-level <trace|debug|info|warn|error>`, `--log-format <pretty|json>`

## Agent Integration

### REST API Endpoints

The server exposes HTTP/REST endpoints that agents can call:

- **List servers**: `GET /servers` — Returns all available MCP endpoints
- **List tools**: `GET /servers/{name}/tools` — List tools from a specific endpoint
- **Call tool**: `POST /tools/{server_name}/{tool_name}` — Execute a tool on a server
- **Health**: `GET /health` — Server health check

### Configuration

Define MCP endpoints in `config.toml`:

```toml
[http]
host = "127.0.0.1"
port = 3000

[mcp]
request_timeout_secs = 30      # Timeout for tool calls (min: 5s)

[[endpoints]]
name = "local-tools"           # Endpoint identifier for agents
type = "local"
command = "node"
args = ["./tools/server.js"]

[[endpoints]]
name = "remote-api"            # Remote HTTP/SSE endpoint
type = "remote"
url = "http://api.example.com/mcp"
```

### Agent Usage Patterns

#### 1. Tool Discovery
Agents discover available tools via REST API:
```bash
curl http://127.0.0.1:3000/servers/{endpoint_name}/tools
```

#### 2. Tool Execution
Agents call tools with structured requests:
```bash
curl -X POST http://127.0.0.1:3000/tools/{endpoint_name}/{tool_name} \
  -H "Content-Type: application/json" \
  -d '{"arg1": "value1", "arg2": "value2"}'
```

#### 3. Server Management
- **Auto-connect**: Endpoints configured with `auto_start = true` are started on server initialization
- **Restart policy**: Failed endpoints can be restarted with configurable delays (`restart_delay_ms`, default 500ms)
- **Status monitoring**: Query `/servers` to check endpoint health and readiness

## Architecture for Agents

### Core Components

| Component | Purpose |
|-----------|---------|
| **API Layer** (`api/`) | HTTP/REST interface for agents to discover and call tools |
| **EndpointManager** | Lifecycle orchestration (start/stop/restart endpoints) |
| **MCP Client** | Protocol handler for communication with tool servers |
| **PathRouter** | Maps HTTP requests to the correct MCP endpoint |
| **Tool Filter** | Include/exclude tools based on agent permissions |

### Concurrency Model

- **Lock-free routing**: `DashMap` for concurrent endpoint management
- **Graceful shutdown**: `CancellationToken` for clean shutdown on signals (SIGTERM, SIGINT)
- **Timeout handling**: Configurable `request_timeout_secs` (default: 30s) for tool execution

### Transport Support

- **Local endpoints**: stdio-based via `rmcp` TokioChildProcess
- **Remote endpoints**: HTTP/SSE via `StreamableHttpClientTransport` with reverse proxy
- **Mixed mode**: Combine local and remote endpoints in single server instance

## Configuration Reference

### Global Settings

```toml
[http]
host = "127.0.0.1"
port = 3000

[logging]
level = "info"                 # trace, debug, info, warn, error
format = "pretty"              # pretty or json

[mcp]
request_timeout_secs = 30      # Tool call timeout (min: 5s)
```

### Endpoint Configuration

```toml
[[endpoints]]
name = "my-endpoint"
path = "/my-endpoint"          # HTTP path prefix
type = "local" | "remote"
auto_start = true              # Auto-connect on startup

# Local endpoint (stdio)
command = "node"
args = ["server.js"]
env = { DEBUG = "true" }

# OR Remote endpoint (HTTP/SSE)
url = "http://api.example.com/mcp"

# Tool filtering (optional)
include_tools = ["tool1", "tool2"]
exclude_tools = ["admin-tool"]
```

## Key Types & API

| Type | Purpose |
|------|---------|
| `EndpointKind` | Runtime enum: `LocalEndpoint` (stdio) or `RemoteEndpoint` (HTTP) |
| `EndpointManager` | Manage endpoint lifecycle (start/stop/restart) |
| `ToolDefinition` | Tool metadata (name, description, schema) |
| `PathRouter` | Route HTTP requests to MCP endpoints |
| `ApiState` | Shared state for HTTP handlers with timeout config |

## Building Agents on Top

### Example: Custom Agent Integration

1. **Discovery phase**: Agent queries `/servers` and `/servers/{name}/tools`
2. **Tool registration**: Agent registers tools from endpoint into its tool registry
3. **Execution**: Agent calls `/tools/{endpoint}/{tool}` when needed
4. **Error handling**: Respect timeout settings and retry policies

### Timeout Considerations

- Default: 30 seconds per tool call
- Minimum: 5 seconds
- Configure in `[mcp]` section for all endpoints
- Agents should implement retry logic for transient failures

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

## Project Structure

```
src/
├── api/                       # HTTP/REST layer for agents
│   ├── handlers.rs            # Request handlers (tool discovery, execution)
│   ├── routes.rs              # HTTP route definitions
│   └── mcp_sse_service.rs     # SSE transport for remote endpoints
├── endpoint/                  # Endpoint lifecycle management
│   ├── manager.rs             # Orchestrate start/stop/restart
│   ├── local.rs               # stdio-based endpoints
│   └── remote.rs              # HTTP/SSE endpoints
├── mcp/                       # MCP protocol
│   ├── client.rs              # Tool server communication
│   ├── bridge.rs              # stdio <-> HTTP bridge
│   └── types.rs               # Protocol types
├── routing/                   # Request routing
│   └── path_router.rs         # Map requests to endpoints
└── config/                    # Configuration loading
```

## Supported Configuration Files

See `config.toml` and examples in `examples/` directory:
- `test-config.toml` — Basic local endpoint
- `test-remote-config.toml` — Remote HTTP endpoint
- `test-docker-config.toml` — Docker-based endpoints
- `test-comprehensive.toml` — Full feature showcase

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| rmcp 0.14 | MCP SDK (protocol) |
| axum 0.8 | HTTP framework |
| tokio 1.49 | Async runtime |
| dashmap 6 | Concurrent hash maps |
| tracing | Structured logging |
| serde / toml | Configuration |

## Debug & Monitoring

Enable structured logging:
```bash
cargo run -- --config config.toml --log-level debug --log-format json
```

Monitor endpoint status:
```bash
curl http://127.0.0.1:3000/servers | jq '.[] | {name: .name, status: .status}'
```
