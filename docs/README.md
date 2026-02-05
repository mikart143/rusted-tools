# Documentation Index

## Overview

This directory contains detailed documentation about the rusted-tools MCP proxy architecture, specifically focusing on how remote MCP server support is implemented.

## Documents

### 1. [REMOTE_MCP_ARCHITECTURE.md](./REMOTE_MCP_ARCHITECTURE.md)

**Complete architecture guide** covering:
- MCP HTTP/SSE protocol explanation
- Reverse proxy implementation details
- Request flow diagrams
- Security considerations
- Comparison: Local vs Remote servers
- Performance characteristics
- Troubleshooting guide
- Future enhancements

**Best for:** Understanding the overall architecture and design decisions

### 2. [REVERSE_PROXY_EXAMPLE.md](./REVERSE_PROXY_EXAMPLE.md)

**Practical walkthrough** with:
- Step-by-step request/response flow
- Real examples with Microsoft Learn API
- Code execution path
- Testing commands
- Configuration examples
- Common troubleshooting scenarios

**Best for:** Learning how requests are processed and testing the implementation

## Quick Start

### Configuration

```toml
[[mcp_servers]]
name = "microsoft-learn"
type = "remote"
url = "https://learn.microsoft.com/api/mcp"
path = "microsoft-learn"
```

### Start the Proxy

```bash
cargo run --release -- --config config.toml
```

### VS Code Setup

```json
{
  "mcpServers": {
    "microsoft-learn": {
      "url": "http://127.0.0.1:3000/mcp/microsoft-learn",
      "type": "http"
    }
  }
}
```

## Architecture Summary

```
VS Code (HTTP/SSE)
    ↓
Rusted-Tools Proxy
    ├─ Local Servers → McpBridgeServer (stdio → SSE)
    └─ Remote Servers → ReverseProxy (HTTP/SSE → HTTP/SSE)
        ↓
Remote MCP Server (HTTP/SSE)
```

## Key Implementation

The core implementation is just ~20 lines in `src/http/mod.rs`:

```rust
match server_info.server_type {
    ServerType::Remote => {
        let remote_server = state.manager.get_remote_server(&server_name)?;
        
        let proxy = ReverseProxy::new(
            &format!("/mcp/{}", path),
            &remote_server.url
        );
        
        app = app.merge(proxy);
    }
}
```

## Why Reverse Proxy?

Remote MCP servers **already speak HTTP/SSE**, so:

❌ **Don't need:** Protocol translation, message parsing, double wrapping  
✅ **Just need:** Forward requests to the right URL

This is what `axum-reverse-proxy` does perfectly!

## Benefits

- **Simple:** ~10 lines of routing code
- **Reliable:** Battle-tested reverse proxy crate
- **Fast:** Minimal overhead, no buffering
- **Transparent:** Preserves all headers and auth
- **Maintainable:** Clear separation of concerns

## Testing

```bash
# Test initialize
curl -X POST http://127.0.0.1:3000/mcp/microsoft-learn/message \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "clientInfo": {"name": "test", "version": "1.0"}
    }
  }'
```

Expected response:
```
event: message
data: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05",...}}
```

## Related Files

### Source Code
- `src/http/mod.rs` - Router setup and proxy configuration
- `src/server/manager.rs` - Server registration and lookup
- `src/server/remote.rs` - RemoteMcpServer struct
- `src/proxy/bridge.rs` - McpBridgeServer (for local servers)

### Configuration
- `config.toml` - Example configuration with local and remote servers
- `test-remote-config.toml` - Test configuration for Microsoft Learn

### Dependencies
- `Cargo.toml` - Project dependencies including `axum-reverse-proxy`

## Further Reading

- [Model Context Protocol Specification](https://spec.modelcontextprotocol.io/)
- [MCP HTTP/SSE Transport Docs](https://spec.modelcontextprotocol.io/specification/basic/transports/#http-with-sse)
- [axum-reverse-proxy Crate](https://docs.rs/axum-reverse-proxy/)
- [Server-Sent Events Spec](https://html.spec.whatwg.org/multipage/server-sent-events.html)

## Contributing

When modifying remote server support:

1. **Keep it simple** - Avoid adding complexity to the proxy layer
2. **Preserve transparency** - Don't parse or modify messages
3. **Test with real servers** - Use Microsoft Learn or other public MCP servers
4. **Update docs** - Keep these documents in sync with code changes

## Questions?

- Architecture questions? Read `REMOTE_MCP_ARCHITECTURE.md`
- Implementation questions? Read `REVERSE_PROXY_EXAMPLE.md`
- Testing issues? Check troubleshooting sections in both docs
- Still stuck? Check server logs and compare with examples

---

**Last Updated:** 2026-02-05  
**Status:** ✅ Stable, production-ready
