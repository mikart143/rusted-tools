# Reverse Proxy Example: Step-by-Step

This document demonstrates how the reverse proxy works with a real example using the Microsoft Learn MCP server.

## Configuration

```toml
# test-remote-config.toml
[[mcp_servers]]
name = "microsoft-learn"
type = "remote"
url = "https://learn.microsoft.com/api/mcp"
path = "microsoft-learn"
```

## The Flow

### Step 1: VS Code Sends Request

VS Code MCP client sends an HTTP POST request:

```http
POST http://127.0.0.1:3000/mcp/microsoft-learn/message HTTP/1.1
Host: 127.0.0.1:3000
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

### Step 2: Proxy Receives and Routes

The rusted-tools proxy:

1. **Receives** request at `/mcp/microsoft-learn/message`
2. **Looks up** server by path `microsoft-learn`
3. **Finds** remote server config: `https://learn.microsoft.com/api/mcp`
4. **Determines** server type is `Remote`
5. **Uses** `ReverseProxy` handler (not `McpBridgeServer`)

### Step 3: axum-reverse-proxy Forwards

The reverse proxy:

1. **Strips** `/mcp/microsoft-learn` prefix
2. **Keeps** `/message` suffix
3. **Constructs** target URL: `https://learn.microsoft.com/api/mcp/message`
4. **Copies** all headers except `Host`
5. **Forwards** request body unchanged
6. **Sends** POST request to remote server

```http
POST https://learn.microsoft.com/api/mcp/message HTTP/1.1
Host: learn.microsoft.com
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

### Step 4: Remote Server Responds

Microsoft Learn MCP server processes the request and responds with SSE:

```http
HTTP/1.1 200 OK
Content-Type: text/event-stream
Transfer-Encoding: chunked

event: message
data: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"logging":{},"prompts":{"listChanged":true},"resources":{"listChanged":true},"tools":{"listChanged":true}},"serverInfo":{"name":"Microsoft Learn MCP Server","version":"1.0.0"}}}


```

### Step 5: Proxy Streams Back

The proxy streams the SSE response back to VS Code without modification:

```http
HTTP/1.1 200 OK
Content-Type: text/event-stream
Transfer-Encoding: chunked

event: message
data: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"logging":{},"prompts":{"listChanged":true},"resources":{"listChanged":true},"tools":{"listChanged":true}},"serverInfo":{"name":"Microsoft Learn MCP Server","version":"1.0.0"}}}


```

### Step 6: VS Code Processes Response

VS Code MCP client:
1. Receives SSE event stream
2. Parses `event: message` lines
3. Extracts JSON from `data:` lines
4. Processes JSON-RPC response
5. Discovers server capabilities and tools

## Code Execution Path

```
┌─────────────────────────────────────────────────────────────────┐
│ src/http/mod.rs: build_router()                                  │
├─────────────────────────────────────────────────────────────────┤
│ for (path, server_name) in routes {                              │
│   match server_info.server_type {                                │
│     ServerType::Remote => {                                       │
│       let remote_server = manager.get_remote_server(&server_name)│
│       let proxy = ReverseProxy::new(                             │
│         "/mcp/microsoft-learn",                                   │
│         "https://learn.microsoft.com/api/mcp"                    │
│       );                                                          │
│       app = app.merge(proxy);  ← Adds route handler             │
│     }                                                             │
│   }                                                               │
│ }                                                                 │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ axum-reverse-proxy crate                                         │
├─────────────────────────────────────────────────────────────────┤
│ - Matches /mcp/microsoft-learn/*                                 │
│ - Extracts remaining path: /message                              │
│ - Builds target: https://learn.microsoft.com/api/mcp/message    │
│ - Creates HTTP client request                                    │
│ - Forwards headers and body                                      │
│ - Streams response back                                          │
└─────────────────────────────────────────────────────────────────┘
```

## Key Implementation Details

### 1. No Message Inspection

The proxy **does not**:
- Parse JSON-RPC messages
- Validate MCP protocol
- Modify request or response
- Buffer the response

### 2. Transparent Forwarding

Everything is forwarded:
- HTTP method (POST, GET, etc.)
- All headers (including Authorization)
- Request body (unchanged)
- Query parameters
- Response status code
- Response headers
- Response body (streamed)

### 3. Path Mapping

```
Client Path:        /mcp/microsoft-learn/message
Proxy Base:         /mcp/microsoft-learn
Remote Base:        https://learn.microsoft.com/api/mcp
Forwarded Path:     /message
Final URL:          https://learn.microsoft.com/api/mcp/message
```

### 4. SSE Streaming

Server-Sent Events are streamed in real-time:
- No buffering
- Chunked transfer encoding
- Keeps connection alive
- Multiple events per response

## Testing the Flow

### Start the Proxy

```bash
cargo run --release -- --config test-remote-config.toml
```

### Test Initialize

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
      "clientInfo": {"name": "test", "version": "1.0"}
    }
  }'
```

### Expected Response

```
event: message
data: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"logging":{},"prompts":{"listChanged":true},"resources":{"listChanged":true},"tools":{"listChanged":true}},"serverInfo":{"name":"Microsoft Learn MCP Server","version":"1.0.0"},"instructions":"..."}}
```

### Test Tool List

```bash
curl -X POST http://127.0.0.1:3000/mcp/microsoft-learn/message \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/list"
  }'
```

### Expected Response

```
event: message
data: {"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"microsoft_docs_search","description":"Search Microsoft documentation","inputSchema":{...}},{"name":"microsoft_code_sample_search",...},{"name":"microsoft_docs_fetch",...}]}}
```

## Comparison: Direct vs Proxied

### Direct Connection

```
VS Code → https://learn.microsoft.com/api/mcp/message
```

### Proxied Connection

```
VS Code → http://127.0.0.1:3000/mcp/microsoft-learn/message → https://learn.microsoft.com/api/mcp/message
```

### Benefits of Proxying

1. **Unified Configuration** - Configure all MCP servers (local + remote) in one place
2. **Path Abstraction** - Use friendly paths like `/mcp/microsoft-learn` instead of full URLs
3. **Authentication Centralization** - Could add auth at proxy level (future feature)
4. **Monitoring** - Can log/track all MCP requests (future feature)
5. **Mixed Mode** - Combine local and remote servers seamlessly
6. **Consistent Interface** - Same URL pattern for all servers

## Why This Works

### The Key Insight

Remote MCP servers **already speak HTTP/SSE**, so we don't need to:
- Wrap them in another layer
- Translate protocols
- Parse messages
- Bridge transports

We just need to **forward requests** to the right place!

### axum-reverse-proxy Does This Perfectly

It handles:
- ✅ All HTTP methods
- ✅ Header forwarding
- ✅ Streaming responses
- ✅ SSE events
- ✅ WebSocket upgrades (if needed)
- ✅ TLS/HTTPS
- ✅ Chunked encoding

## Architecture Benefits

### 1. Simplicity
- No custom HTTP client code
- No SSE parsing/generation
- No protocol translation
- Just routing configuration

### 2. Reliability
- Battle-tested reverse proxy implementation
- Proper HTTP/1.1 and HTTP/2 support
- Correct header handling
- Streaming without buffering

### 3. Performance
- Minimal overhead (~1-5ms)
- No message parsing
- No buffering
- Direct streaming

### 4. Maintainability
- Less code to maintain
- Clear separation: local (bridge) vs remote (proxy)
- Easy to understand
- Well-documented dependencies

## Troubleshooting

### Check Proxy Logs

```bash
cargo run --release -- --config test-remote-config.toml
```

Look for:
```
Setting up HTTP reverse proxy for remote server microsoft-learn at /mcp/microsoft-learn → https://learn.microsoft.com/api/mcp
```

### Test Remote Server Directly

```bash
curl -X POST https://learn.microsoft.com/api/mcp/message \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
```

### Test Proxy Routing

```bash
# Check server is registered
curl http://127.0.0.1:3000/servers

# Should show:
# {
#   "servers": [{
#     "name": "microsoft-learn",
#     "path": "microsoft-learn",
#     "type": "remote",
#     "status": "stopped"
#   }]
# }
```

### Common Issues

**Issue:** 404 Not Found
- Check path in config matches URL: `/mcp/{path}`
- Verify server is registered in `/servers` endpoint

**Issue:** Connection Refused
- Check remote server URL is accessible
- Test with curl directly

**Issue:** CORS Errors
- CORS middleware is enabled in router
- Check browser console for specific errors

**Issue:** Empty Response
- Check remote server is returning SSE format
- Verify Content-Type is `text/event-stream`

## Summary

The reverse proxy implementation is:

1. **Simple** - Just forwards HTTP requests
2. **Transparent** - Preserves all headers and streaming
3. **Reliable** - Uses battle-tested `axum-reverse-proxy`
4. **Efficient** - Minimal overhead, no buffering
5. **Maintainable** - Clean architecture, well-separated concerns

This is **much better** than the original approach of trying to wrap remote HTTP/SSE servers in another MCP bridge layer!
