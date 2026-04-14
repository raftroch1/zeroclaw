# ZeroClaw Integration Analysis Report

**Date:** 2026-04-08  
**Branch:** `cleanup/remove-dead-mcp`  
**Purpose:** Comprehensive technical analysis for Lana Sterling's autonomous agent runtime  
**Feeds into:** 4 downstream implementation tasks

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Current State Assessment](#2-current-state-assessment)
3. [Root Cause Analysis: Fix-Restart-Revert Loop](#3-root-cause-analysis)
4. [Hermes Integration Analysis](#4-hermes-integration-analysis)
5. [ByteRover MCP Integration Analysis](#5-byterover-mcp-integration-analysis)
6. [Gap Analysis Table](#6-gap-analysis-table)
7. [Dependency Map](#7-dependency-map)
8. [Implementation Roadmap](#8-implementation-roadmap)

---

## 1. Architecture Overview

### System Architecture (ASCII)

```
┌─────────────────────────────────────────────────────────────────┐
│                     HOST MACHINE                                │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              ZEROCLAW CONTAINER (Docker)                  │   │
│  │                                                          │   │
│  │  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐   │   │
│  │  │ Agent Loop   │  │ SecurityPolicy│  │  Tool Registry│   │   │
│  │  │ (agent.rs)   │──│ (policy.rs)  │──│  (mod.rs)     │   │   │
│  │  └──────┬───────┘  └──────────────┘  └───────┬───────┘   │   │
│  │         │                                     │           │   │
│  │         │      ┌──────────────────────────────┤           │   │
│  │         ▼      ▼                              ▼           │   │
│  │  ┌───────────────┐  ┌──────────────┐  ┌─────────────┐    │   │
│  │  │ DelegateTool   │  │ ShellTool    │  │ MCP Servers │    │   │
│  │  │ (delegate.rs)  │  │ (shell.rs)   │  │ (config)    │    │   │
│  │  └───────┬────────┘  └──────┬───────┘  └──────┬──────┘    │   │
│  │          │                  │                  │           │   │
│  │          ▼                  ▼                  ▼           │   │
│  │  In-process sub-agents  bash/sh calls   HTTP to MCP       │   │
│  │  (same binary, same     (gated by       servers           │   │
│  │   container)             allowlist)                        │   │
│  │                                                           │   │
│  │  Volumes: /zeroclaw-data/workspace (workspace)            │   │
│  │  Ports:   3000 (gateway)                                  │   │
│  └───────────────────────────────────────────────────────────┘   │
│                          │                                       │
│              ┌───────────┴────────────┐                          │
│              ▼                        ▼                          │
│  ┌────────────────────┐  ┌──────────────────────┐               │
│  │ Hermes Agent       │  │ ByteRover Daemon      │               │
│  │ (separate container │  │ (host or container)   │               │
│  │  or host process)   │  │ Socket.IO + MCP stdio │               │
│  └────────────────────┘  └──────────────────────┘               │
└─────────────────────────────────────────────────────────────────┘
```

### Three Repositories at a Glance

| Repo | Language | Role | Key Entry Point |
|------|----------|------|-----------------|
| **ZeroClaw** | Rust | Agent runtime with sandboxed execution | `src/main.rs` → gateway/CLI → agent loop |
| **Hermes Agent** | Python | Multi-agent orchestration with delegate_task | `cli.py` / `hermes_cli/main.py` |
| **ByteRover CLI** | TypeScript | Knowledge management with MCP server | `brv mcp` → `ByteRoverMcpServer` (stdio) |

---

## 2. Current State Assessment

### 2.1 ZeroClaw (Rust Agent Runtime)

**Branch:** `cleanup/remove-dead-mcp`

#### Core Architecture

ZeroClaw is a Rust monolith (~150+ source files) structured as:

- **`src/agent/`** — Agent loop (`loop_.rs`), message dispatch, memory loading, prompt generation
- **`src/security/policy.rs`** — SecurityPolicy: command allowlist, risk classification, path validation, rate limiting
- **`src/tools/`** — 30+ tool implementations including `delegate.rs`, `shell.rs`, `file_read.rs`, `file_write.rs`
- **`src/config/schema.rs`** — Full config schema including `MCPConfig`, `MCPServerConfig`, `DelegateAgentConfig`
- **`src/runtime/`** — Execution runtime: native, Docker, WASM
- **`src/providers/`** — LLM providers: OpenRouter, Anthropic, Ollama, Gemini, OpenAI, etc.

#### Key Finding: No `hermes.rs` Exists

**Critical correction:** The task context references `src/tools/hermes.rs` — this file does **not exist** in the repo. The actual delegation system is `src/tools/delegate.rs` implementing a `DelegateTool` that spawns **in-process** sub-agents (not container-based). This is ZeroClaw's native multi-agent system, not a Hermes integration.

```rust
// src/tools/delegate.rs — The ACTUAL delegation system
pub struct DelegateTool {
    agents: Arc<HashMap<String, DelegateAgentConfig>>,
    security: Arc<SecurityPolicy>,
    fallback_credential: Option<String>,
    provider_runtime_options: providers::ProviderRuntimeOptions,
    depth: u32,                              // recursion depth tracking
    parent_tools: Arc<Vec<Arc<dyn Tool>>>,   // tools available to sub-agents
    multimodal_config: crate::config::MultimodalConfig,
}
```

The DelegateTool runs sub-agents **in the same process** using different provider/model configs. It does NOT:
- Spawn Docker containers
- Execute external Hermes binaries
- Use file-based task delegation
- Create isolated filesystem namespaces

#### MCP Integration (Current State)

ZeroClaw has a basic MCP config but **no generic MCP client**. The current approach is hardcoded name-matching:

```rust
// src/tools/mod.rs — Lines 284-302 (actual code)
if root_config.mcp.enabled {
    for server in &root_config.mcp.servers {
        if server.enabled && server.transport == "http" {
            match server.name.as_str() {
                "mem0-brain-surgeon" => {
                    tool_arcs.push(Arc::new(Mem0MemoryTool::with_config(
                        security.clone(),
                        server.url.clone(),
                        server.timeout_secs,
                    )));
                }
                _ => {
                    // Other MCP servers can be added here
                }
            }
        }
    }
}
```

**MCPServerConfig** supports both `stdio` and `http` transports in the schema, but only `http` + `"mem0-brain-surgeon"` name is wired up. There is no:
- Generic MCP client (stdio or SSE)
- Tool discovery from MCP servers
- Dynamic tool registration from MCP
- McpRegistry abstraction

The `MCPServerConfig` schema:

```rust
pub struct MCPServerConfig {
    pub name: String,
    pub transport: String,        // "stdio" | "http"
    pub url: String,              // for http transport
    pub command: String,          // for stdio transport
    pub args: Vec<String>,        // for stdio transport
    pub env: Option<HashMap<String, String>>,
    pub enabled: bool,
    pub headers: Option<HashMap<String, String>>,
    pub timeout_secs: u64,
}
```

#### SecurityPolicy Analysis

The policy in `policy.rs` is **strict by design** but configurable via `config.toml`:

**Hardcoded blocks (cannot be bypassed):**
- Subshell operators: `` ` ``, `$(`, `${`, `<(`, `>(`
- Output redirections: `>`, `>>` (unquoted)
- Background chaining: standalone `&`
- `tee` command

**Configurable via `allowed_commands`:**
- Base command must match allowlist
- Arguments validated for `find -exec` and `git config`
- Risk classification (Low/Medium/High) with approval gates

**The docker-config/config.toml** already has a very permissive allowlist including `pip`, `pip3`, `python3`, `bash`, `sh`, `chmod`, `mkdir`, and 100+ commands. The problem is this config was written inside the container but never committed.

#### Dockerfile Architecture

Two-stage build:
1. **Builder stage** — Rust compilation with cargo caches
2. **Dev stage** — Debian trixie-slim with runtime deps (git, python3, nodejs, docker-cli)
3. **Release stage** — Distroless (no shell at all)

The dev stage copies `dev/config.template.toml` which is a **minimal** Ollama config:

```toml
# dev/config.template.toml — MINIMAL, not production-ready
api_key = "http://host.docker.internal:11434"
default_provider = "ollama"
default_model = "llama3.2"
[gateway]
port = 3000
host = "[::]"
allow_public_bind = true
```

The comprehensive `docker-config/config.toml` exists in the repo but is **not copied into the Docker image**. This is a root cause of the fix-restart-revert problem.

### 2.2 Hermes Agent (Python Multi-Agent System)

**Key discovery:** Hermes Agent is a Python application, not Rust.

#### Architecture

- **Entry:** `hermes_cli/main.py` → argparse CLI with subcommands (gateway, cron, doctor, etc.)
- **Agent:** `run_agent.py` — ~4000+ line AIAgent class
- **Delegation:** `tools/delegate_tool.py` — `delegate_task()` function
- **Config:** `HERMES_HOME` env var → `config.yaml` (YAML, not TOML)
- **Docker:** Debian 13.4 image, entrypoint bootstraps config into `/opt/data`

#### delegate_task Tool (Actual Interface)

```python
# tools/delegate_tool.py
def delegate_task(
    goal: Optional[str] = None,
    context: Optional[str] = None,
    toolsets: Optional[List[str]] = None,
    tasks: Optional[List[Dict[str, Any]]] = None,
    max_iterations: Optional[int] = None,
    acp_command: Optional[str] = None,
    acp_args: Optional[List[str]] = None,
    parent_agent=None,
) -> str:
```

**How it works:**
1. Creates child `AIAgent` instances **in-process** (not containers)
2. Each child gets: fresh conversation, own task_id, restricted toolset, focused system prompt
3. Blocked tools: `delegate_task`, `clarify`, `memory`, `send_message`, `execute_code`
4. Max depth: 2 (parent → child → grandchild rejected)
5. Max concurrent: 3 children
6. Children inherit parent's provider/model or override via `delegation` config section

**Hermes CLI flags:**

```
hermes                          # Interactive chat
hermes gateway                  # HTTP gateway
hermes --profile <name>         # Use named profile (sets HERMES_HOME)
hermes -m <model>               # Override model
hermes --provider <provider>    # Override provider
```

**Key env var:** `HERMES_HOME` controls config directory (default: `~/.hermes`)

**Config structure** (`config.yaml`):
```yaml
delegation:
  max_iterations: 50
  default_toolsets: ["terminal", "file", "web"]
  # model: "google/gemini-3-flash-preview"  # optional override
  # provider: "openrouter"                   # optional override
```

#### Docker Deployment

```dockerfile
FROM debian:13.4
COPY . /opt/hermes
WORKDIR /opt/hermes
RUN pip install --no-cache-dir -e ".[all]" --break-system-packages
ENV HERMES_HOME=/opt/data
VOLUME [ "/opt/data" ]
ENTRYPOINT [ "/opt/hermes/docker/entrypoint.sh" ]
```

The entrypoint copies `.env.example`, `cli-config.yaml.example`, and `SOUL.md` into `HERMES_HOME` if they don't exist.

### 2.3 ByteRover CLI (TypeScript MCP Server)

#### Architecture

ByteRover is a TypeScript CLI (oclif framework) with a daemon + MCP server architecture:

- **Daemon:** Long-running `brv` process managing context trees (Socket.IO transport)
- **MCP Server:** Spawned by `brv mcp` command, communicates with daemon via Socket.IO
- **Transport:** MCP uses **stdio** (stdin/stdout) for parent agent communication
- **Tools exposed:** `brv-query` and `brv-curate`

#### MCP Server Details

```typescript
// src/server/infra/mcp/mcp-server.ts
class ByteRoverMcpServer {
    constructor(config: McpServerConfig) { ... }
    async start(): Promise<void> {
        // 1. Connect to brv daemon via Socket.IO
        // 2. Create McpServer instance (@modelcontextprotocol/sdk)
        // 3. Register brv-query and brv-curate tools
        // 4. Connect via StdioServerTransport (stdin/stdout)
    }
}
```

**Tool: `brv-query`**
- Input: `{ query: string, cwd?: string }`
- Behavior: Creates `task:create` event, waits for `llmservice:response` + `task:completed`
- Timeout: 5 minutes
- Returns: Aggregated response text

**Tool: `brv-curate`**
- Input: `{ context: string, cwd?: string }` or `{ files: string[], cwd?: string }` or `{ folder: string, cwd?: string }`
- Behavior: Fire-and-forget (creates task, returns immediately)
- Returns: Success acknowledgment

#### How External Systems Connect

ByteRover's `McpConnector` supports 20+ agents with auto-install configs. The typical pattern:

```json
// Agent's MCP config (e.g., for Claude Code, Cursor)
{
  "mcpServers": {
    "byterover": {
      "command": "brv",
      "args": ["mcp"],
      "cwd": "/path/to/project"
    }
  }
}
```

**Transport:** stdio — the calling agent spawns `brv mcp` as a child process and communicates over stdin/stdout using the MCP protocol.

---

## 3. Root Cause Analysis: Fix-Restart-Revert Loop

### The Problem

```
┌─────────────────────────────────────────────────────────┐
│                    THE REVERT CYCLE                       │
│                                                          │
│   1. Agent discovers issue (e.g., policy too strict)     │
│   2. Agent fixes files INSIDE running container          │
│      - Edits /app/src/security/policy.rs                 │
│      - Edits /zeroclaw-data/.zeroclaw/config.toml        │
│   3. Fix works! Until...                                 │
│   4. Container restarts (crash, redeploy, docker restart)│
│   5. ALL CHANGES LOST — image layer is read-only         │
│   6. Next agent rediscovers same issues                  │
│   7. GOTO 1                                              │
└─────────────────────────────────────────────────────────┘
```

### Root Causes (Layered)

#### Cause 1: Docker Image Immutability (Primary)

Docker containers are ephemeral by design. The filesystem is:
- **Image layers (read-only):** Source code, compiled binary, default config
- **Container layer (writable but ephemeral):** Changes made at runtime
- **Volumes (persistent):** Only `/zeroclaw-data/workspace` is mounted

When the container restarts, the writable layer is discarded. Changes to source code, config files, or installed packages vanish.

**The critical gap:** `config.toml` lives inside the image at `/zeroclaw-data/.zeroclaw/config.toml`. It is NOT on a volume. Therefore:

```
Container writes to: /zeroclaw-data/.zeroclaw/config.toml  ← EPHEMERAL (container layer)
Volume persists:     /zeroclaw-data/workspace/              ← PERSISTENT
```

#### Cause 2: Config Not Baked Into Image

The Dockerfile copies `dev/config.template.toml` (12-line Ollama minimal config) instead of `docker-config/config.toml` (comprehensive production config with 200+ lines, correct allowlists, MCP servers, identity, etc.):

```dockerfile
# Dockerfile line 86 — THE PROBLEM
COPY dev/config.template.toml /zeroclaw-data/.zeroclaw/config.toml
```

Should be:
```dockerfile
COPY docker-config/config.toml /zeroclaw-data/.zeroclaw/config.toml
```

#### Cause 3: Source Code Changes Require Rebuild

Fixes to `policy.rs` (e.g., adding redirect-safe shell operators) are **source code changes**. They require:
1. Edit the `.rs` file in the **git repo on the host**
2. `cargo build --release` (or `docker build`)
3. Rebuild and restart the container

An agent running INSIDE the container cannot do steps 1-3. It can edit the source file in the container, but:
- The Rust binary is already compiled — editing `.rs` does nothing at runtime
- Even if it could recompile, changes are lost on restart

#### Cause 4: No Config Volume Mount

The fix is to mount the config directory as a volume:

```yaml
# docker-compose.yml (what should exist)
volumes:
  - ./docker-config/config.toml:/zeroclaw-data/.zeroclaw/config.toml
  - ./workspace:/zeroclaw-data/workspace
```

This way config changes persist across restarts. But this doesn't exist.

### Summary: Why It's Structurally Unsolvable From Inside

| What | Can agent fix inside container? | Persists? |
|------|-------------------------------|-----------|
| `config.toml` settings | ✅ Yes (write to file) | ❌ No (not on volume) |
| `policy.rs` security logic | ❌ No (compiled binary) | ❌ No |
| `Dockerfile` build steps | ❌ No (not in container) | ❌ No |
| pip/npm packages | ✅ Yes (install at runtime) | ❌ No |
| Volume-mounted files | ✅ Yes | ✅ Yes |

**The solution must come from outside:** Edit the repo, rebuild the image, or mount configs as volumes.

---

## 4. Hermes Integration Analysis

### Current State: No Integration Exists

There is **no** `hermes.rs` in ZeroClaw. The `delegate.rs` DelegateTool is ZeroClaw's own in-process delegation system. It does not invoke Hermes.

### What Would Integration Look Like?

Two approaches:

#### Approach A: HTTP Gateway Bridge (Recommended)

```
┌───────────────────┐    HTTP/REST     ┌──────────────────┐
│ ZeroClaw           │───────────────→ │ Hermes Gateway    │
│ (DelegateTool or   │                 │ (hermes gateway)  │
│  new HermesTool)   │←───────────────│ port 8080         │
└───────────────────┘   JSON response  └──────────────────┘
```

**How:** Run Hermes in a separate container with `hermes gateway`, and ZeroClaw calls it via HTTP.

**Hermes gateway** provides a REST API for submitting prompts and receiving responses. ZeroClaw would need a new `HermesTool` or could use the existing `http_request` tool.

**Pros:** Clean separation, no Docker-in-Docker, works with ZeroClaw's existing HTTP capabilities
**Cons:** Requires running Hermes as a persistent service

#### Approach B: CLI Invocation via Shell

```
┌───────────────────┐    shell exec    ┌──────────────────┐
│ ZeroClaw           │───────────────→ │ hermes CLI        │
│ (ShellTool)        │  hermes -m ...  │ (one-shot mode)   │
│                    │←───────────────│                    │
└───────────────────┘   stdout result  └──────────────────┘
```

**How:** ZeroClaw's shell tool runs `hermes` CLI commands. Hermes would need to be installed in the container or reachable.

**Requirements if Hermes runs in ZeroClaw's container:**
- Python 3.x + Hermes pip package installed in Docker image
- `HERMES_HOME` set to a persistent volume
- `hermes` added to `allowed_commands` in security policy

**Requirements if Hermes runs in a sibling container:**
- Docker socket mounted or Docker CLI available
- Network connectivity between containers
- ZeroClaw allowed to run `docker exec` commands

### Critical Integration Details

#### Hermes CLI Flags

```bash
hermes --profile <name>    # Sets HERMES_HOME to ~/.hermes/profiles/<name>
hermes -m <model>          # Override model
hermes --provider <prov>   # Override provider
```

**NOT** `--config-dir` — the context mentioned fixing `--agent-config` to `--config-dir`, but Hermes uses `--profile` / `HERMES_HOME` env var, not `--config-dir`.

#### Volume Requirements for Hermes Container

```yaml
# Hermes needs persistent state
volumes:
  - hermes-data:/opt/data           # HERMES_HOME: config, memories, sessions
  - shared-workspace:/workspace      # Shared workspace with ZeroClaw
environment:
  - HERMES_HOME=/opt/data
```

#### delegate_task Configuration

```yaml
# Hermes config.yaml delegation section
delegation:
  max_iterations: 50
  default_toolsets: ["terminal", "file", "web"]
  model: "anthropic/claude-sonnet-4-20250514"
  provider: "openrouter"
```

### Recommendation

**Use Approach A (HTTP Gateway Bridge)** because:
1. No Docker-in-Docker complexity
2. Clean process isolation
3. ZeroClaw already supports HTTP tools
4. Hermes gateway is production-ready
5. Config persists naturally in Hermes's own volume

---

## 5. ByteRover MCP Integration Analysis

### Current State: No Integration Exists

ZeroClaw's MCP handling is hardcoded to `"mem0-brain-surgeon"` name matching. There is no generic MCP client that could connect to ByteRover.

### ByteRover's MCP Interface

**Transport:** stdio (StdioServerTransport)  
**Command:** `brv mcp`  
**Tools:** `brv-query`, `brv-curate`

#### brv-query Tool Schema
```json
{
  "name": "brv-query",
  "description": "Query the context tree",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": { "type": "string", "description": "Natural language query" },
      "cwd": { "type": "string", "description": "Working directory (required in global mode)" }
    },
    "required": ["query"]
  }
}
```

#### brv-curate Tool Schema
```json
{
  "name": "brv-curate",
  "description": "Curate context to the tree",
  "inputSchema": {
    "type": "object",
    "properties": {
      "context": { "type": "string", "description": "Context text to store" },
      "files": { "type": "array", "items": { "type": "string" }, "description": "File paths to curate" },
      "folder": { "type": "string", "description": "Folder path to pack and curate" },
      "cwd": { "type": "string", "description": "Working directory" }
    }
  }
}
```

### Integration Path: ZeroClaw ↔ ByteRover

#### Option 1: Stdio MCP Client (Proper, Recommended)

Build a generic MCP stdio client in ZeroClaw that can:
1. Spawn `brv mcp` as a child process
2. Communicate via stdin/stdout using MCP protocol
3. Discover tools dynamically via `tools/list`
4. Route tool calls from the agent to the MCP server

**Config would look like:**
```toml
[[mcp.servers]]
name = "byterover"
transport = "stdio"
command = "brv"
args = ["mcp"]
enabled = true
env = { BRV_PROJECT_ROOT = "/zeroclaw-data/workspace" }
```

**What ZeroClaw needs:**
```rust
// New: src/tools/mcp_client.rs (or src/mcp/client.rs)
pub struct McpStdioClient {
    process: tokio::process::Child,
    stdin: tokio::io::BufWriter<ChildStdin>,
    stdout: tokio::io::BufReader<ChildStdout>,
}

impl McpStdioClient {
    pub async fn spawn(config: &MCPServerConfig) -> Result<Self> { ... }
    pub async fn list_tools(&mut self) -> Result<Vec<McpToolSchema>> { ... }
    pub async fn call_tool(&mut self, name: &str, args: Value) -> Result<Value> { ... }
}

// Wrapper to expose MCP tools as ZeroClaw Tools
pub struct McpToolProxy {
    client: Arc<Mutex<McpStdioClient>>,
    tool_schema: McpToolSchema,
    security: Arc<SecurityPolicy>,
}

impl Tool for McpToolProxy {
    fn name(&self) -> &str { &self.tool_schema.name }
    fn description(&self) -> &str { &self.tool_schema.description }
    fn parameters_schema(&self) -> Value { self.tool_schema.input_schema.clone() }
    async fn execute(&self, args: Value) -> Result<ToolResult> { ... }
}
```

**Integration in tools/mod.rs:**
```rust
// Replace hardcoded name matching with dynamic discovery
if root_config.mcp.enabled {
    for server_config in &root_config.mcp.servers {
        if !server_config.enabled { continue; }
        match server_config.transport.as_str() {
            "stdio" => {
                let client = McpStdioClient::spawn(server_config).await?;
                let tools = client.list_tools().await?;
                for tool_schema in tools {
                    tool_arcs.push(Arc::new(McpToolProxy::new(
                        client.clone(), tool_schema, security.clone()
                    )));
                }
            }
            "http" => {
                // Existing HTTP handling (or also genericize)
            }
        }
    }
}
```

#### Option 2: HTTP Bridge (Simpler, Less Ideal)

Run ByteRover's daemon with an HTTP endpoint and use ZeroClaw's existing HTTP tool:

```toml
[[mcp.servers]]
name = "byterover-bridge"
transport = "http"
url = "http://host.docker.internal:8002"
enabled = true
```

This requires a custom HTTP-to-MCP bridge running on the host, which adds complexity.

#### Option 3: Shell Wrapper (Quick Hack)

Create shell scripts that invoke `brv` CLI directly:

```bash
# In container, if brv is installed
brv query "What do we know about authentication?"
brv curate --context "New finding: auth uses JWT tokens"
```

ZeroClaw's shell tool could run these if `brv` is added to `allowed_commands`.

### Requirements for ByteRover in Docker

ByteRover needs:
1. **Node.js runtime** (already in ZeroClaw dev image)
2. **`brv` CLI installed** globally (`npm install -g @campfirein/byterover-cli`)
3. **Running `brv` daemon** — the MCP server connects to the daemon via Socket.IO
4. **Project initialized** — `.brv/config.json` in workspace root

**Challenge:** ByteRover's MCP server is a child of the daemon, not standalone. The daemon must be running for the MCP server to function. This means either:
- Run `brv` daemon inside ZeroClaw container (adds complexity)
- Run daemon on host, configure MCP server to connect to it via network

### Recommendation

**Start with Option 3 (Shell Wrapper)** for quick wins, then build toward **Option 1 (Stdio MCP Client)** for the proper integration. Option 1 is the correct long-term architecture but requires significant Rust development.

---

## 6. Gap Analysis Table

| Component | What Exists | What's Partially Done | What Needs Fresh Work | Blocked By |
|-----------|------------|----------------------|----------------------|------------|
| **config.toml persistence** | `docker-config/config.toml` in repo | Config is comprehensive but not baked into image | Mount as volume or COPY in Dockerfile | Nothing — pure DevOps |
| **Security policy (shell chaining)** | `policy.rs` blocks `>`, `>>`, `\|` globally | Comprehensive quote-aware parser exists | Add workspace-safe redirect whitelist | Requires Rust code change + rebuild |
| **Security policy (pip/packages)** | `docker-config/config.toml` has pip in allowlist | Config exists but not in image | Bake config into image | Config persistence fix |
| **DelegateTool (in-process)** | Fully implemented with depth limits, agentic mode | Complete and tested | None for in-process delegation | — |
| **Hermes integration** | Nothing | No code exists | New `HermesTool` or HTTP bridge | Hermes running as service |
| **MCP generic client (stdio)** | `MCPServerConfig` has stdio fields | Schema exists but no client code | Full `McpStdioClient` implementation | Significant Rust dev |
| **MCP generic client (HTTP)** | Hardcoded to `mem0-brain-surgeon` | `Mem0MemoryTool` works via HTTP | Generalize to any HTTP MCP server | Moderate Rust dev |
| **ByteRover MCP integration** | Nothing | — | Install brv, configure MCP server | MCP client + brv daemon |
| **Voice API (port 8001)** | Unknown current state | May have been running before | Rebuild/redeploy | Unclear requirements |
| **Pushover notifications** | `tools/pushover.rs` exists | Tool code exists | `.env` config with API key | Config persistence |
| **Docker image rebuild** | Dockerfile exists and is functional | — | Rebuild with correct config | Host access to run `docker build` |
| **Volume mounts** | Workspace volume exists | Only workspace is mounted | Add config volume, shared workspace | `docker-compose.yml` update |

---

## 7. Dependency Map

```
                    ┌──────────────────┐
                    │ 1. FIX DOCKERFILE │
                    │ & CONFIG PERSISTENCE │
                    └────────┬─────────┘
                             │
                    ┌────────┴─────────┐
                    │                  │
          ┌─────────▼──────┐  ┌────────▼───────────┐
          │ 2. FIX SECURITY │  │ 3. DOCKER-COMPOSE   │
          │    POLICY       │  │    VOLUME MOUNTS     │
          └─────────┬──────┘  └────────┬───────────┘
                    │                  │
                    └────────┬─────────┘
                             │
               ┌─────────────┴──────────────┐
               │                            │
    ┌──────────▼──────────┐   ┌─────────────▼────────────┐
    │ 4a. HERMES GATEWAY   │   │ 4b. MCP STDIO CLIENT     │
    │     INTEGRATION      │   │     (ByteRover)           │
    └──────────┬──────────┘   └─────────────┬────────────┘
               │                            │
               └────────────┬───────────────┘
                            │
                   ┌────────▼──────────┐
                   │ 5. VOICE API &     │
                   │    PUSHOVER        │
                   └───────────────────┘
```

### Dependency Details

| Step | Depends On | Blocks |
|------|-----------|--------|
| 1. Fix Dockerfile & config | Nothing | 2, 3, 4a, 4b |
| 2. Fix security policy | 1 (for testing) | 4a (shell commands for Hermes) |
| 3. Docker-compose volumes | 1 | 4a, 4b (runtime config) |
| 4a. Hermes integration | 1, 2, 3 | 5 (partially) |
| 4b. MCP stdio client | 1, 3 | ByteRover integration |
| 5. Voice API & Pushover | 1, 3 | Nothing |

---

## 8. Implementation Roadmap

### Task 1: Fix Dockerfile & Config Persistence (Foundation)

**Complexity:** Low  
**Estimated effort:** 1-2 hours  
**Files to change:** `Dockerfile`, `docker-compose.yml` (new), `docker-config/config.toml`

**Actions:**
1. Update `Dockerfile` dev stage to COPY `docker-config/config.toml` instead of `dev/config.template.toml`
2. Create `docker-compose.yml` with proper volume mounts:
   ```yaml
   services:
     zeroclaw:
       build:
         context: .
         target: dev
       volumes:
         - ./docker-config/config.toml:/zeroclaw-data/.zeroclaw/config.toml
         - zeroclaw-workspace:/zeroclaw-data/workspace
       ports:
         - "3000:3000"
       environment:
         - API_KEY=${API_KEY}
   ```
3. Ensure `docker-config/config.toml` has all production settings
4. Add `.env.example` for required environment variables

**This permanently breaks the fix-restart-revert cycle.**

### Task 2: Fix Security Policy for Shell Chaining

**Complexity:** Medium  
**Estimated effort:** 2-4 hours  
**Files to change:** `src/security/policy.rs`, `docker-config/config.toml`

**Actions:**
1. Add workspace-safe redirect whitelist to `is_command_allowed()`:
   ```rust
   // Allow > and >> ONLY when target path is inside workspace
   fn is_redirect_target_safe(&self, command: &str) -> bool {
       // Parse redirect targets from command
       // Check each target against workspace_dir
       // Return true only if ALL targets are workspace-safe
   }
   ```
2. Modify the blanket `contains_unquoted_char(command, '>')` block to call workspace-safe check
3. Ensure `config.toml` allowlist includes all needed commands
4. Add tests for the new behavior
5. Rebuild Docker image

**Key architectural note:** The redirect block exists for good security reasons. The fix should be surgical: allow redirects only to paths within `workspace_dir`, not blanket-allow them.

### Task 3: Hermes Integration (HTTP Gateway Bridge)

**Complexity:** Medium-High  
**Estimated effort:** 4-8 hours  
**Files to change:** New `src/tools/hermes_gateway.rs`, `src/tools/mod.rs`, `src/config/schema.rs`, `docker-compose.yml`

**Actions:**
1. Add `HermesConfig` to `config/schema.rs`:
   ```rust
   pub struct HermesConfig {
       pub enabled: bool,
       pub gateway_url: String,  // e.g., "http://hermes:8080"
       pub timeout_secs: u64,
   }
   ```
2. Implement `HermesGatewayTool` in new `src/tools/hermes_gateway.rs`:
   - HTTP client to Hermes gateway
   - Maps ZeroClaw delegate-style calls to Hermes API
   - Handles streaming responses
3. Register in `tools/mod.rs`
4. Add Hermes container to `docker-compose.yml`:
   ```yaml
   hermes:
     build: ./path/to/hermes-agent
     volumes:
       - hermes-data:/opt/data
       - zeroclaw-workspace:/workspace
     environment:
       - HERMES_HOME=/opt/data
     ports:
       - "8080:8080"
   ```
5. Configure shared workspace volume between ZeroClaw and Hermes

### Task 4: ByteRover MCP Integration

**Complexity:** High  
**Estimated effort:** 8-16 hours (full stdio client) or 2-4 hours (shell wrapper)

#### Phase 1: Shell Wrapper (Quick)
1. Install `brv` in Docker image
2. Add `brv` to `allowed_commands`
3. Agent can use shell tool to run `brv query "..."` and `brv curate "..."`

#### Phase 2: Generic MCP Stdio Client (Proper)
1. Implement `McpStdioClient` — spawn process, JSON-RPC over stdio
2. Implement `McpToolProxy` — wraps discovered MCP tools as ZeroClaw tools
3. Dynamic tool discovery via MCP `tools/list` method
4. Integration in `tools/mod.rs` for all `transport = "stdio"` servers
5. Tests with mock MCP server

**MCP Protocol Reference:**
```
Client → Server: {"jsonrpc":"2.0","method":"initialize","params":{...},"id":1}
Server → Client: {"jsonrpc":"2.0","result":{"capabilities":{"tools":{}},...},"id":1}
Client → Server: {"jsonrpc":"2.0","method":"tools/list","params":{},"id":2}
Server → Client: {"jsonrpc":"2.0","result":{"tools":[{name,description,inputSchema},...]},"id":2}
Client → Server: {"jsonrpc":"2.0","method":"tools/call","params":{"name":"brv-query","arguments":{...}},"id":3}
Server → Client: {"jsonrpc":"2.0","result":{"content":[{"type":"text","text":"..."}]},"id":3}
```

---

## Appendix A: File Reference

### ZeroClaw Key Files
| File | Purpose |
|------|---------|
| `src/security/policy.rs` | Command allowlist, risk classification, path validation |
| `src/tools/delegate.rs` | In-process sub-agent delegation (NOT Hermes) |
| `src/tools/mod.rs` | Tool registry, MCP server wiring |
| `src/config/schema.rs` | Full config schema (MCPConfig, DelegateAgentConfig, etc.) |
| `Dockerfile` | Multi-stage build (builder → dev → release) |
| `docker-config/config.toml` | Production config (not baked into image!) |
| `dev/config.template.toml` | Minimal dev config (currently baked into image) |

### Hermes Agent Key Files
| File | Purpose |
|------|---------|
| `tools/delegate_tool.py` | `delegate_task()` — child agent spawning |
| `hermes_cli/main.py` | CLI entry point with argparse |
| `run_agent.py` | AIAgent class (~4000 lines) |
| `cli-config.yaml.example` | Full config reference (delegation section) |
| `docker/entrypoint.sh` | Docker bootstrap script |
| `Dockerfile` | Debian 13.4 + Python + Node |

### ByteRover CLI Key Files
| File | Purpose |
|------|---------|
| `src/server/infra/mcp/mcp-server.ts` | MCP server class (stdio transport) |
| `src/server/infra/mcp/tools/brv-query-tool.ts` | Query tool registration |
| `src/server/infra/mcp/tools/brv-curate-tool.ts` | Curate tool registration |
| `src/oclif/commands/mcp.ts` | `brv mcp` CLI command |
| `src/server/infra/connectors/mcp/mcp-connector.ts` | Agent config auto-install |
| `src/server/infra/connectors/mcp/mcp-connector-config.ts` | Per-agent MCP configs |

## Appendix B: Critical Corrections to Task Context

1. **`hermes.rs` does not exist** — The delegation system is `delegate.rs` and is ZeroClaw-native, not a Hermes integration
2. **`--agent-config` vs `--config-dir`** — Hermes uses `--profile` / `HERMES_HOME` env var, neither of these flags
3. **Git Bash path conversion** — Not relevant; ZeroClaw runs in Linux Docker containers
4. **"McpRegistry"** — Does not exist as a struct. MCP integration is hardcoded name-matching in `tools/mod.rs`
5. **`docker-config/config.toml`** — Already exists in the repo with comprehensive settings (pip, bash, sh, chmod, etc. in allowlist), but is not COPY'd into the Docker image
