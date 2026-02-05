# Remote MCP Server Architecture

## Overview

This document explains how the rusted-tools proxy handles remote MCP servers using HTTP/SSE reverse proxy.

## MCP HTTP/SSE Protocol

The Model Context Protocol over HTTP uses Server-Sent Events (SSE) for bidirectional communication:

### Request Flow
```
Client                    Proxy                     Remote MCP Server
  |                         |                              |
  |-- POST /mcp/path ------>|                              |
  |    (JSON-RPC request)   |                              |
  |                         |-- Forward POST ------------>|
  |                         |    (same request)            |
  |                         |                              |
  |                         |<-- SSE Response ------------|
  |<-- SSE Response --------|    (event: message)         |
  |    (proxied through)    |    (data: JSON-RPC)         |
  |                         |                              |
```

### MCP Message Format

MCP uses JSON-RPC 2.0 over SSE:

**Request (HTTP POST):**
```json
POST /mcp/microsoft-learn/message HTTP/1.1
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {
      "name": "vscode",
      "version": "1.0"
    }
  }
}
```

**Response (Server-Sent Events):**
```
HTTP/1.1 200 OK
Content-Type: text/event-stream

event: message
data: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{...}}}

event: message  
data: {"jsonrpc":"2.0","method":"notifications/tools/list_changed"}
```

## Reverse Proxy Implementation

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      VS Code MCP Client                          │
│  - Sends JSON-RPC over HTTP POST                                 │
│  - Receives SSE streaming responses                              │
└──────────────────────┬──────────────────────────────────────────┘
                       │
                       │ HTTP/SSE
                       │
┌──────────────────────▼──────────────────────────────────────────┐
│              Rusted-Tools Proxy (Port 3000)                      │
│                                                                   │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Router Layer (Axum)                                        │ │
│  │  - Routes: /mcp/{path}/*                                    │ │
│  └────────────────┬───────────────────────────────────────────┘ │
│                   │                                               │
│  ┌────────────────▼───────────────────────────────────────────┐ │
│  │  Server Type Detection                                      │ │
│  │  - Local → McpBridgeServer (stdio → SSE)                   │ │
│  │  - Remote → ReverseProxy (HTTP/SSE → HTTP/SSE)             │ │
│  └────────────────┬───────────────────────────────────────────┘ │
│                   │                                               │
│  ┌────────────────▼───────────────────────────────────────────┐ │
│  │  axum-reverse-proxy                                         │ │
│  │  - Forwards all HTTP methods                                │ │
│  │  - Preserves headers (auth, content-type)                   │ │
│  │  - Streams SSE responses                                    │ │
│  │  - Handles WebSocket upgrades                               │ │
│  └────────────────┬───────────────────────────────────────────┘ │
└───────────────────┼─────────────────────────────────────────────┘
                    │
                    │ HTTP/SSE (forwarded)
                    │
┌───────────────────▼─────────────────────────────────────────────┐
│         Remote MCP Server (e.g., Microsoft Learn)               │
│  - Receives standard MCP HTTP/SSE requests                      │
│  - Returns SSE responses with JSON-RPC messages                 │
│  - URL: https://learn.microsoft.com/api/mcp                     │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation Details

### 1. Configuration

```toml
[[mcp_servers]]
name = "microsoft-learn"
type = "remote"
url = "https://learn.microsoft.com/api/mcp"  # Base URL of remote server
path = "microsoft-learn"                      # Local proxy path
```

### 2. Router Setup (src/http/mod.rs)

```rust
match server_info.server_type {
    ServerType::Remote => {
        // Get remote server config
        let remote_server = state.manager.get_remote_server(&server_name)?;
        
        // Create reverse proxy that forwards all requests
        let proxy = ReverseProxy::new(
            &format!("/mcp/{}", path),  // Local path: /mcp/microsoft-learn
            &remote_server.url           // Remote URL: https://learn.microsoft.com/api/mcp
        );
        
        // Merge proxy router into main app
        app = app.merge(proxy);
    }
}
```

### 3. How axum-reverse-proxy Works

The `axum-reverse-proxy` crate provides transparent HTTP proxying:

**Key Features:**
- **Path Forwarding:** `/mcp/microsoft-learn/message` → `https://learn.microsoft.com/api/mcp/message`
- **Header Preservation:** All request headers forwarded (authorization, content-type, etc.)
- **Streaming:** SSE responses streamed back without buffering
- **Method Support:** GET, POST, PUT, DELETE, etc.
- **TLS Support:** HTTPS connections via rustls

**Under the hood:**
1. Receives HTTP request at `/mcp/{path}/*`
2. Strips the `/mcp/{path}` prefix
3. Forwards remaining path to remote URL
4. Copies all headers (except Host)
5. Forwards request body
6. Streams response back to client
7. Preserves response headers and status codes

### 4. SSE Streaming

Server-Sent Events are streamed through the proxy:

```
Client POST request
    ↓
Proxy receives POST
    ↓
Forward to remote server
    ↓
Remote server responds with:
    Transfer-Encoding: chunked
    Content-Type: text/event-stream
    ↓
Proxy streams chunks back to client
    ↓
Client receives SSE events in real-time
```

## Why Reverse Proxy vs Custom Implementation?

### ❌ Original Approach (Failed)
```
VS Code HTTP/SSE → Proxy HTTP Server → StreamableHttpClientTransport (SSE) 
    → McpBridgeServer → SSE → VS Code
```
**Problem:** Double SSE wrapping, empty messages, protocol mismatch

### ✅ Current Approach (Working)
```
VS Code HTTP/SSE → axum-reverse-proxy → Remote Server HTTP/SSE
```
**Benefits:**
- Direct forwarding (no protocol translation)
- Preserves all headers and content-types
- Native SSE streaming
- Simple and reliable

## Request Examples

### Initialize Request

**VS Code sends:**
```bash
curl -X POST http://127.0.0.1:3000/mcp/microsoft-learn/message \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "clientInfo": {"name": "vscode", "version": "1.0"}
    }
  }'
```

**Proxy forwards to:**
```
POST https://learn.microsoft.com/api/mcp/message
Content-Type: application/json
{same body}
```

**Remote server responds with:**
```
HTTP/1.1 200 OK
Content-Type: text/event-stream

event: message
data: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05",...}}
```

**Proxy streams back:**
```
HTTP/1.1 200 OK
Content-Type: text/event-stream

event: message
data: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05",...}}
```

### Tool List Request

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list"
}
```

**Response:**
```
event: message
data: {"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"microsoft_docs_search",...}]}}
```

### Tool Call Request

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "microsoft_docs_search",
    "arguments": {
      "query": "rust async programming"
    }
  }
}
```

**Response:**
```
event: message
data: {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"..."}]}}
```

## Key Components

### 1. Remote Server Config (src/server/remote.rs)
```rust
pub struct RemoteMcpServer {
    pub path: String,  // URL path for routing
    pub url: String,   // Full URL of remote server
}
```

### 2. Server Manager (src/server/manager.rs)
- Registers remote servers from config
- Stores RemoteMcpServer instances
- Provides access via `get_remote_server()`

### 3. HTTP Router (src/http/mod.rs)
- Detects server type (local vs remote)
- Creates appropriate handler:
  - Local: `McpBridgeServer` (stdio → SSE)
  - Remote: `ReverseProxy` (HTTP/SSE → HTTP/SSE)

### 4. Reverse Proxy (axum-reverse-proxy crate)
- Handles all HTTP forwarding
- Streams SSE responses
- Preserves headers and status codes

## Security Considerations

### What's Forwarded
- ✅ All HTTP headers (including Authorization)
- ✅ Request body
- ✅ Query parameters
- ✅ HTTP method

### What's Modified
- ❌ Host header (changed to remote server)
- ❌ X-Forwarded-* headers (added by proxy)

### Authentication
The proxy is **transparent** - authentication is handled by the remote server:
- Bearer tokens passed through
- API keys passed through
- OAuth headers passed through

**Example with authentication:**
```bash
# VS Code sends with auth header
Authorization: Bearer abc123

# Proxy forwards with same header
POST https://learn.microsoft.com/api/mcp/message
Authorization: Bearer abc123
```

## Limitations

### 1. No Tool Filtering
Remote servers don't support tool allowlist/blocklist filtering because:
- Would require parsing and modifying MCP messages
- Breaks message signatures if present
- Remote servers are explicitly configured (already trusted)
- Filtering should be done at remote server level

### 2. No Request Inspection
The proxy forwards requests transparently without:
- Message parsing
- Content validation
- Request modification
- Response caching

### 3. Connection Management
- No connection pooling configured (relies on HTTP client defaults)
- No retry logic (VS Code handles retries)
- No circuit breaking (fails through to client)

## Comparison: Local vs Remote Servers

| Feature | Local (stdio) | Remote (HTTP/SSE) |
|---------|---------------|-------------------|
| **Protocol** | stdin/stdout | HTTP + SSE |
| **Bridge** | McpBridgeServer | ReverseProxy |
| **Translation** | stdio → SSE | None (pass-through) |
| **Tool Filtering** | ✅ Supported | ❌ Not supported |
| **Auto-start** | ✅ Supported | ❌ N/A |
| **Process Management** | ✅ Managed | ❌ External |
| **Authentication** | N/A | ✅ Forwarded |

## Testing

### Quick Test
```bash
# Start proxy
cargo run --release -- --config test-remote-config.toml

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

### Expected Output
```
event: message
data: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{...}}}
```

## VS Code Configuration

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

## Troubleshooting

### Issue: Empty messages
**Cause:** Double SSE wrapping (old architecture)  
**Fix:** Use reverse proxy (current implementation)

### Issue: Content-type errors
**Cause:** SSE not properly forwarded  
**Fix:** Ensure `axum-reverse-proxy` is used for remote servers

### Issue: 404 Not Found
**Cause:** Path not registered or incorrect  
**Fix:** Check config `path` field and verify `/mcp/{path}` route exists

### Issue: Connection refused
**Cause:** Remote server not accessible  
**Fix:** Test remote URL directly with curl

### Issue: CORS errors
**Cause:** Missing CORS headers  
**Fix:** Enable CORS layer in router (already configured)

## Performance

### Latency
- **Added latency:** ~1-5ms (proxy overhead)
- **Network:** Same as direct connection to remote server
- **Streaming:** Real-time (no buffering)

### Throughput
- **Concurrent connections:** Limited by Tokio async runtime
- **Streaming:** Full HTTP/2 support
- **Memory:** Minimal (streaming without buffering)

## Future Enhancements

Possible improvements:
1. Connection pooling configuration
2. Request/response logging
3. Metrics collection (latency, error rates)
4. Circuit breaker pattern
5. Response caching (for idempotent requests)
6. Load balancing (multiple remote servers)
7. Health checks for remote servers
8. Request rate limiting

## References

- [Model Context Protocol Specification](https://spec.modelcontextprotocol.io/)
- [MCP HTTP/SSE Transport](https://spec.modelcontextprotocol.io/specification/basic/transports/#http-with-sse)
- [axum-reverse-proxy crate](https://docs.rs/axum-reverse-proxy/)
- [Server-Sent Events (SSE) Spec](https://html.spec.whatwg.org/multipage/server-sent-events.html)
