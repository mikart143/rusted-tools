# 🦀 Rusted-Tools MCP Proxy

A high-performance Model Context Protocol (MCP) proxy server written in Rust that provides unified access to multiple MCP servers through a REST API.

## ✨ Features

- 🚀 **Multi-Server Management**: Run and manage multiple MCP servers simultaneously
- 🔧 **Multiple Server Types**: Support for Node.js, Docker, and any stdio-based MCP servers
- 🌐 **REST API**: Access MCP tools through simple HTTP endpoints
- ⚡ **High Performance**: Built with Rust and Tokio for maximum efficiency
- 🔄 **Auto-Restart**: Automatic server recovery on failure
- 📊 **Server Monitoring**: Health checks and status tracking
- 🛡️ **Tool Filtering**: Optional allowlist/blocklist for tool access control

## 🎯 Current Status

✅ **Fully Working:**
- Local MCP server management (stdio-based)
- Tool discovery and listing
- Tool execution via REST API
- Docker container support
- Node.js MCP server support
- Multi-server orchestration

⚠️ **Not Yet Implemented:**
- MCP HTTP/SSE transport (required for VS Code integration)
- Remote MCP server support
- Server-to-server notifications

## 📦 Installation

### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs))
- Docker (optional, for Docker-based MCP servers)
- Node.js (optional, for Node-based MCP servers)

### Build from Source

```bash
git clone https://github.com/YOUR_USERNAME/rusted-tools
cd rusted-tools
cargo build --release
```

The binary will be at `./target/release/rusted-tools`

## 🚀 Quick Start

### 1. Create a Configuration File

**config.toml:**
```toml
[server]
host = "127.0.0.1"
port = 3000

# Memory/Knowledge Graph Server (Node.js)
[[mcp_servers]]
name = "memory"
type = "local"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]
path = "memory"
auto_start = true
restart_on_failure = true

# Web Fetch Server (Docker)
[[mcp_servers]]
name = "fetch"
type = "local"
command = "docker"
args = ["run", "--rm", "-i", "mcp/fetch"]
path = "fetch"
auto_start = true
restart_on_failure = false
```

### 2. Start the Proxy

```bash
./target/release/rusted-tools --config config.toml
```

### 3. Use the REST API

**List All Servers:**
```bash
curl http://localhost:3000/api/servers | jq .
```

**List Tools on a Server:**
```bash
curl http://localhost:3000/mcp/memory/tools | jq .
```

**Call a Tool:**
```bash
curl -X POST http://localhost:3000/mcp/memory/tools/call \
  -H "Content-Type: application/json" \
  -d '{
    "name": "create_entities",
    "arguments": {
      "entities": [{
        "name": "Example",
        "entityType": "Demo",
        "observations": ["Test observation"]
      }]
    }
  }' | jq .
```

## 📚 API Documentation

### Management Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/info` | Server information |
| GET | `/api/servers` | List all configured servers |
| GET | `/api/servers/{name}/status` | Get server status |
| POST | `/api/servers/{name}/start` | Start a server |
| POST | `/api/servers/{name}/stop` | Stop a server |
| POST | `/api/servers/{name}/restart` | Restart a server |

### MCP Tool Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/mcp/{path}/tools` | List available tools |
| POST | `/mcp/{path}/tools/call` | Execute a tool |

### Example Responses

**List Tools:**
```json
{
  "server": "memory",
  "tool_count": 9,
  "filter_active": false,
  "tools": [
    {
      "name": "create_entities",
      "description": "Create multiple new entities in the knowledge graph",
      "input_schema": { ... }
    },
    ...
  ]
}
```

**Call Tool:**
```json
{
  "content": [
    {
      "type": "text",
      "text": "Entity created successfully"
    }
  ],
  "is_error": false
}
```

## 🔧 Configuration Reference

### Server Configuration

```toml
[server]
host = "127.0.0.1"    # Listen address
port = 3000            # Listen port
```

### MCP Server Configuration

```toml
[[mcp_servers]]
name = "server-name"           # Unique identifier
type = "local"                 # Server type (currently only "local" supported)
command = "npx"                # Command to execute
args = ["-y", "package"]       # Command arguments
path = "api-path"              # URL path for this server
auto_start = true              # Start on proxy startup
restart_on_failure = true      # Auto-restart if crashed

# Optional: Tool filtering
[mcp_servers.tools]
mode = "allowlist"             # or "blocklist"
patterns = ["create_*", "read_*"]
```

## 📖 Examples

See the `/examples` directory for:
- `test-comprehensive.toml` - Multi-server setup
- `test-docker-config.toml` - Docker-based servers
- API usage examples

## 🧪 Testing

Run the test suite:
```bash
cargo test
```

Run a comprehensive integration test:
```bash
./target/release/rusted-tools --config test-comprehensive.toml
```

Test the API:
```bash
# In another terminal, run the test script
./scripts/test-api.sh
```

## ⚠️ VS Code Integration

**Important:** VS Code cannot currently connect to this proxy because it requires MCP HTTP/SSE transport, which is not yet implemented.

**Workarounds:**
1. Configure VS Code to connect directly to MCP servers (recommended)
2. Use the REST API with curl/scripts
3. Use the MCP Inspector tool

See [VSCODE_USAGE.md](VSCODE_USAGE.md) for details.

## 🗺️ Roadmap

### Planned Features

- [ ] MCP HTTP/SSE transport support
- [ ] Remote MCP server support
- [ ] WebSocket transport
- [ ] Authentication and authorization
- [ ] Rate limiting
- [ ] Request/response caching
- [ ] Prometheus metrics
- [ ] Docker Compose setup
- [ ] Kubernetes deployment manifests
- [ ] Web-based dashboard

## 🤝 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## 📝 License

MIT License - see [LICENSE](LICENSE) file for details

## 🙏 Acknowledgments

- Built with [rmcp](https://github.com/modelcontextprotocol/rust-sdk) - Official Rust MCP SDK
- Inspired by the [Model Context Protocol](https://modelcontextprotocol.io)
- Thanks to Anthropic for creating the MCP specification

## 📧 Contact

- Author: Michał Kruczek <mikart143@gmail.com>
- Issues: [GitHub Issues](https://github.com/YOUR_USERNAME/rusted-tools/issues)

## 🌟 Star History

If you find this project useful, please consider giving it a star! ⭐
