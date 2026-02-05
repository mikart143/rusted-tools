# Using rusted-tools MCP Proxy with VS Code

## Current Limitation

The rusted-tools proxy currently does **NOT** support the MCP HTTP/SSE transport protocol that VS Code uses. 

When VS Code tries to connect, it will fail with:
```
404 status sending message to http://127.0.0.1:3000/mcp/server-name
```

## Workaround Options

### Option 1: Connect Directly to MCP Servers (Recommended)

Instead of using the proxy, configure VS Code to connect directly to MCP servers:

**.vscode/mcp-settings.json:**
```json
{
  "mcpServers": {
    "memory": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-memory"]
    },
    "fetch": {
      "command": "docker",
      "args": ["run", "--rm", "-i", "mcp/fetch"]
    }
  }
}
```

### Option 2: Use the REST API

The proxy provides REST endpoints for tool operations:

**List Tools:**
```bash
GET http://127.0.0.1:3000/mcp/{server-path}/tools
```

**Call a Tool:**
```bash
POST http://127.0.0.1:3000/mcp/{server-path}/tools/call
Content-Type: application/json

{
  "name": "tool_name",
  "arguments": { ... }
}
```

**Example:**
```bash
# List tools on memory server
curl http://localhost:3000/mcp/memory/tools | jq .

# Call a tool
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

### Option 3: Use MCP Inspector

The [MCP Inspector](https://github.com/modelcontextprotocol/inspector) provides a UI for testing MCP servers and can connect through the proxy's REST API.

## Future Enhancement

Full MCP HTTP/SSE transport support is planned for a future release. This will enable:
- Native VS Code integration
- Bidirectional communication
- Server notifications
- Full MCP protocol compliance

## Configuration

**config.toml:**
```toml
[server]
host = "127.0.0.1"
port = 3000

[[mcp_servers]]
name = "memory"
type = "local"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]
path = "memory"
auto_start = true

[[mcp_servers]]
name = "fetch"
type = "local"
command = "docker"
args = ["run", "--rm", "-i", "mcp/fetch"]
path = "fetch"
auto_start = true
```

## Testing

See the comprehensive test in `test-comprehensive.toml` for a working example with multiple servers.
