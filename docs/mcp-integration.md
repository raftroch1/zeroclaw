# MCP Integration Guide

ZeroClaw supports the **Model Context Protocol (MCP)** for integrating external
tool servers. Any MCP-compliant server can be connected via stdio or HTTP
transport, and its tools are automatically discovered and exposed to the LLM.

## Architecture

```
┌──────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   LLM Agent  │────▶│  McpTool (×N)    │────▶│  MCP Server     │
│              │     │  (Tool trait)     │     │  (stdio/http)   │
└──────────────┘     └──────────────────┘     └─────────────────┘
                            │
                     ┌──────┴───────┐
                     │  McpClient   │  (trait)
                     └──────┬───────┘
                      ┌─────┴──────┐
                      │            │
              ┌───────┴──┐  ┌─────┴──────┐
              │ Stdio    │  │ HTTP       │
              │ Client   │  │ (legacy)   │
              └──────────┘  └────────────┘
```

**Key concepts:**

- **`McpClient` trait** (`src/mcp/client.rs`): Generic interface for MCP
  transport implementations. Defines `initialize()`, `list_tools()`, and
  `call_tool()`.
- **`StdioMcpClient`** (`src/mcp/stdio.rs`): Spawns the MCP server as a child
  process and communicates via newline-delimited JSON-RPC 2.0 over stdin/stdout.
- **`McpTool`** (`src/mcp/tools.rs`): Wraps a single MCP server tool, implementing
  ZeroClaw's `Tool` trait. Created automatically during tool discovery.
- **`protocol.rs`** (`src/mcp/protocol.rs`): MCP/JSON-RPC message types for
  initialization, tool listing, and tool invocation.

## Quick Start

### 1. Configure MCP servers in `config.toml`

```toml
[mcp]
enabled = true

# Example: ByteRover code search
[[mcp.servers]]
name = "byterover"
transport = "stdio"
command = "brv"
args = ["mcp"]
enabled = true
timeout_secs = 30

# Example: Custom MCP server with environment variables
[[mcp.servers]]
name = "my-custom-server"
transport = "stdio"
command = "node"
args = ["dist/index.js"]
enabled = true
timeout_secs = 60

[mcp.servers.env]
API_KEY = "your-api-key"
DATABASE_URL = "postgres://..."
```

### 2. Start ZeroClaw

On startup, ZeroClaw will:
1. Spawn each enabled stdio MCP server process
2. Perform the MCP handshake (`initialize` + `notifications/initialized`)
3. Discover available tools via `tools/list`
4. Register each discovered tool in the LLM tool registry

If an MCP server fails to start or initialize, a warning is logged and ZeroClaw
continues with the remaining tools.

### 3. Use the tools

Discovered tools are available to the LLM like any other ZeroClaw tool. Tool
names follow the convention:

- **Single server**: Tool names use the MCP tool name with hyphens replaced by
  underscores (e.g., `brv-query` → `brv_query`).
- **Multiple servers**: Tool names are prefixed with the server name to avoid
  collisions (e.g., `byterover__brv_query`).

## Configuration Reference

### `[mcp]` Section

| Field     | Type   | Default | Description                       |
|-----------|--------|---------|-----------------------------------|
| `enabled` | bool   | `false` | Master switch for MCP integration |

### `[[mcp.servers]]` Entries

| Field         | Type              | Default   | Description                                       |
|---------------|-------------------|-----------|---------------------------------------------------|
| `name`        | string            | required  | Unique server identifier                          |
| `transport`   | string            | `"stdio"` | Transport type: `"stdio"` or `"http"`             |
| `command`     | string            | `""`      | Executable for stdio transport                    |
| `args`        | array of strings  | `[]`      | Arguments for stdio command                       |
| `env`         | map string→string | `{}`      | Environment variables for stdio process           |
| `url`         | string            | `""`      | Server URL for HTTP transport                     |
| `headers`     | map string→string | `{}`      | Custom HTTP headers for HTTP transport            |
| `enabled`     | bool              | `true`    | Whether this server is active                     |
| `timeout_secs`| u64               | `30`      | Request timeout in seconds                        |

## ByteRover Setup

[ByteRover](https://github.com/campfirein/byterover-cli) provides AI-powered
code search and context curation via MCP.

### Prerequisites

1. **Install ByteRover CLI**: `npm install -g @campfirein/byterover-cli`
2. **Start the daemon**: `brv daemon start` (must be running before ZeroClaw)
3. **Index your codebase**: `brv index /path/to/your/project`

### Configuration

```toml
[mcp]
enabled = true

[[mcp.servers]]
name = "byterover"
transport = "stdio"
command = "brv"
args = ["mcp"]
enabled = true
timeout_secs = 30

[mcp.servers.env]
BRV_PROJECT_ROOT = "/zeroclaw-data/workspace"
```

### Available Tools

ByteRover exposes two tools via MCP:

- **`brv_query`** (or `byterover__brv_query`): Query the context tree using
  natural language. Returns relevant code snippets with context.

  Parameters:
  - `query` (string, required): Natural language search query
  - `cwd` (string, optional): Working directory (required in global mode)

- **`brv_curate`** (or `byterover__brv_curate`): Curate context to the tree.
  Store new findings, index files, or pack folders.

  Parameters:
  - `context` (string, optional): Context text to store
  - `files` (array of strings, optional): File paths to curate
  - `folder` (string, optional): Folder path to pack and curate
  - `cwd` (string, optional): Working directory

### Docker Deployment Notes

ByteRover's MCP server is a child of the `brv` daemon — the daemon must be
running for the MCP server to function. Options for Docker:

1. **Run daemon inside ZeroClaw container** — add `brv daemon start` to your
   entrypoint script before ZeroClaw starts.
2. **Run daemon on host** — configure the MCP server to connect to the host
   daemon via network (set `BRV_PROJECT_ROOT` appropriately).
3. **Sidecar container** — run `brv` in a separate container and use stdio
   transport with `docker exec` as the command.

## Adding a New MCP Server

1. Ensure the server speaks MCP over stdio (newline-delimited JSON-RPC 2.0)
2. Add a `[[mcp.servers]]` entry to `config.toml`
3. Restart ZeroClaw — tools are discovered automatically

No code changes are needed. The generic MCP client handles:
- Process lifecycle (spawn, stdin/stdout communication, cleanup)
- MCP handshake (initialize → initialized notification)
- Tool discovery (`tools/list`)
- Tool invocation (`tools/call`) with parameter marshaling
- Response parsing and error handling

## Troubleshooting

### MCP server fails to start

```
WARN Failed to initialize MCP stdio server, skipping
```

**Causes:**
- Command not found (check `command` field and `PATH`)
- Missing environment variables (check `env` field)
- Server crashes on startup (check server logs)

**Debug:** Run the command manually to verify it works:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}' | brv mcp
```

### Tool discovery returns empty list

- Verify the server supports `tools/list`
- Check if the server requires additional initialization
- Increase `timeout_secs` if the server is slow to respond

### Tool invocation errors

- Check that arguments match the tool's input schema
- Look for `MCP server error` messages in logs
- Verify the server's backing service is running (e.g., `brv daemon` for ByteRover)

### Timeout errors

- Increase `timeout_secs` in the server configuration
- Check if the MCP server process is still running
- Verify network connectivity for HTTP transport servers

## Files Changed (Phase 4)

| File | Change |
|------|--------|
| `src/mcp/mod.rs` | New module definition and exports |
| `src/mcp/protocol.rs` | MCP/JSON-RPC protocol message types |
| `src/mcp/client.rs` | Generic `McpClient` trait |
| `src/mcp/stdio.rs` | Stdio transport implementation (`StdioMcpClient`) |
| `src/mcp/tools.rs` | MCP tool wrapper (`McpTool` implementing `Tool` trait) |
| `src/lib.rs` | Added `pub mod mcp` |
| `src/tools/mod.rs` | Replaced hardcoded MCP with generic client discovery |
| `docker-config/config.toml` | Added ByteRover example configuration |
| `docs/mcp-integration.md` | This documentation |
