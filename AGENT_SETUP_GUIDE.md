# ZeroClaw Agent Setup Guide

This guide helps you configure a ZeroClaw agent with:
- Telegram bot integration
- Shell and file tools (workspace-scoped)
- z.ai (GLM model) as LLM provider
- MCP server integration (via z.ai)

## Prerequisites

### 1. Get Your API Keys

#### Telegram Bot Token
1. Open Telegram and search for @BotFather
2. Send `/newbot` command
3. Choose a name (e.g., "zeroclaw_bot")
4. Copy the bot token (starts with `123456789:ABC...`)

#### z.ai API Key
1. Visit [https://open.bigmodel.cn/dev/apikey](https://open.bigmodel.cn/dev/apikey)
2. Sign up or log in
3. Create a new API key
4. Copy your API key

### 2. Get Your Telegram User ID

To secure your bot, add only yourself to the allowlist:

1. In Telegram, send a message to your bot
2. The bot will log a warning with your user ID (e.g., "Ignoring message from unauthorized user: 123456789")
3. Note this ID for the configuration below

## Configuration

### Step 1: Create/Update `~/.zeroclaw/config.toml`

```toml
# ─────────────────────────────────────────────────────────────
# Provider Configuration (z.ai / GLM)
# ─────────────────────────────────────────────────────────────

# IMPORTANT: Use "zai" for Coding Plan (Pro subscription)
# - "zai" uses Coding Plan endpoint (works with Pro subscription quota)
# - "glm" uses Standard API endpoint (requires prepaid balance)
default_provider = "zai"

# Use GLM-4.7 model (flagship, 200K context, 128K output, excellent tool calling)
# Available with all GLM Coding Plan tiers (Lite, Max, Pro)
default_model = "glm-4.7"

# Your z.ai API key
api_key = "your_zai_api_key_here"

# Optional: z.ai API URL (usually not needed, auto-selected by provider)
# Coding Plan: https://api.z.ai/api/coding/paas/v4
# Standard API: https://api.z.ai/api/paas/v4

# Optional: Set default temperature
default_temperature = 0.7

# ─────────────────────────────────────────────────────────────
# Telegram Channel Configuration
# ─────────────────────────────────────────────────────────────

[channels_config.telegram]
# Your BotFather bot token
bot_token = "123456789:ABCdefGHIjklMNOpqrsTUVwxyz"

# Allowed users (deny-by-default when empty)
# Add your numeric user ID from step 2
# Use "*" to allow all (not recommended for production)
allowed_users = ["123456789"]

# Optional: Streaming mode (Off, Partial)
# Off = send complete response at once
# Partial = progressive updates via message edits
stream_mode = "Off"

# Optional: Draft update interval (ms, default 1000)
# Reduces rate limit pressure when streaming
draft_update_interval_ms = 1000

# Optional: Interrupt on new message (default false)
# Cancels in-flight request on new message from same sender
interrupt_on_new_message = false

# Optional: Mention-only mode (default false)
# Only respond to @mentions in group chats
mention_only = false

# ─────────────────────────────────────────────────────────────
# Autonomy & Tools Configuration
# ─────────────────────────────────────────────────────────────

[autonomy]
# Autonomy level: readonly, supervised (default), full
level = "supervised"

# Restrict all file/shell operations to workspace only
workspace_only = true

# Allowlist of shell commands (empty = no shell access)
# Common commands: ["git", "cargo", "ls", "cat", "grep", "curl", "cat", "echo", "cd", "pwd", "find", "head", "tail", "sed", "awk"]
allowed_commands = [
    "ls",
    "cat",
    "grep",
    "curl",
    "cat",
    "echo",
    "cd",
    "pwd",
    "find",
    "head",
    "tail",
    "sed",
    "awk",
    "mkdir",
    "rm",
    "cp",
    "mv",
    "touch",
    "wc",
    "sort",
    "uniq",
    "git",
    "cargo",
    "rustc",
    "sh",
    "bash",
    "python3",
    "node",
    "npm",
    "pip"
]

# Explicit path denylist (default includes system dirs)
# Additional paths to block:
forbidden_paths = []

# Optional: Require approval for medium-risk commands (default true)
require_approval_for_medium_risk = true

# Optional: Block high-risk commands even if allowlisted (default true)
block_high_risk_commands = true

# ─────────────────────────────────────────────────────────────
# Memory Configuration
# ─────────────────────────────────────────────────────────────

[memory]
# Backend: sqlite (default), postgres, markdown, none
backend = "sqlite"

# Enable auto-save of conversations
auto_save = true

# Embedding provider: none, openai, custom:https://...
embedding_provider = "none"

# Vector search weight (0.0-1.0)
vector_weight = 0.7

# Keyword search weight (0.0-1.0)
keyword_weight = 0.3

# ─────────────────────────────────────────────────────────────
# Web Search (z.ai MCP integration)
# ─────────────────────────────────────────────────────────────

[web_search]
# Enable web search tool
enabled = true

# Provider: duckduckgo (free), brave (requires API key)
provider = "brave"

# Brave API key (if using Brave)
brave_api_key = "your_brave_api_key_here"

# Max results per search (1-10)
max_results = 5

# Request timeout in seconds
timeout_secs = 30

# ─────────────────────────────────────────────────────────────
# Security & Secrets
# ─────────────────────────────────────────────────────────────

[secrets]
# Encrypt API keys at rest (recommended)
encrypt = true
```

### Step 2: Test Configuration

```bash
# Verify configuration is loaded
zeroclaw status

# Check channel health
zeroclaw channel doctor

# Start the agent daemon
zeroclaw daemon
```

### Step 3: Initialize Workspace

```bash
# Create a workspace directory for your agent
mkdir -p ~/zeroclaw-workspace

# The agent will operate within this directory
# All file read/write operations are scoped here
```

## Security Considerations

### Telegram Security
- **Never commit bot tokens** to version control
- **Use allowlists**: Only add known user IDs to `allowed_users`
- **Empty allowlist = deny all**: No unauthorized access
- **Use mention_only in groups**: Only respond when @mentioned

### Workspace Security
- **workspace_only = true**: All file operations are restricted to workspace
- **Default forbidden paths**: `/etc`, `/root`, `/proc`, `/sys`, `~/.ssh`, `~/.gnupg`, `~/.aws`
- **Custom forbidden paths**: Add sensitive directories to `forbidden_paths`

### Tool Security
- **allowed_commands**: Only specified commands can be executed
- **require_approval_for_medium_risk**: Medium-risk commands need explicit approval
- **block_high_risk_commands**: High-risk commands are always blocked

## Running the Agent

### Start Daemon Mode
```bash
# Start in background (recommended for Telegram bot)
zeroclaw daemon

# Run in foreground (for debugging)
zeroclaw daemon --foreground
```

### Monitor Logs
```bash
# View real-time logs
journalctl -u zeroclaw -f

# Or on macOS/Linux without systemd:
tail -f ~/.local/state/zeroclaw/zeroclaw.log
```

## Tool Capabilities

### Available Tools (with above configuration)

1. **Shell Tool**: Execute commands in `~/zeroclaw-workspace`
   - Allowlisted commands only
   - Workspace-scoped paths
   - Approval workflow for medium/high risk

2. **File Tools**: Read, write, list files
   - Workspace-scoped only
   - Path traversal blocked
   - Symlink escape detection

3. **HTTP Request Tool**: curl-like functionality
   - Allowlisted domains only
   - Size limits enforced
   - Timeout protection

4. **Memory Tools**: Recall, save, search
   - SQLite hybrid search (vector + keyword)
   - Markdown file backend
   - Auto-save enabled

5. **Web Search Tool**: Via z.ai MCP integration
   - Brave or DuckDuckGo search
   - Configurable results count
   - Timeout protection

## MCP Server Integration

### z.ai MCP Servers

ZeroClaw integrates with z.ai MCP servers for enhanced capabilities:

1. **Web Search MCP**: `https://api.z.ai/api/mcp/web_search_prime/mcp`
   - Requires z.ai API key
   - Real-time web search
   - Configurable result limits

2. **Configuration Note**: Set `web_search.enabled = true` and `web_search.provider = "brave"` in config.toml
   - z.ai handles MCP communication internally
   - No separate MCP server configuration needed

### Alternative: External MCP Configuration

If you need to connect to other MCP servers, you can:

1. Use `delegate` tool to route to MCP-enabled sub-agents
2. Configure delegate agents in `[agents]` section of config.toml

```toml
[agents.web-search]
provider = "glm"
model = "glm-4"
api_key = "your_zai_api_key"

[agents.web-search.system_prompt]
"""You are a web search assistant. Use the provided search tool to find current information."""
```

## Troubleshooting

### Telegram Not Responding
```bash
# Check if daemon is running
zeroclaw status

# Check Telegram channel logs
journalctl -u zeroclaw | grep telegram

# Verify bot token in config
grep bot_token ~/.zeroclaw/config.toml
```

### Tools Not Working
```bash
# Check autonomy configuration
zeroclaw status | grep -A 10 autonomy

# Verify workspace directory exists
ls -la ~/zeroclaw-workspace

# Test shell tool manually
zeroclaw agent -m "Run: ls -la" --dry-run
```

### z.ai API Errors

#### Error 1113: "Insufficient balance or no resource package"

**Cause**: Using "glm" provider (Standard API) instead of "zai" provider (Coding Plan).

**Solution**: Change provider from "glm" to "zai" in config:
```bash
# Edit config
nano ~/.zeroclaw/config.toml

# Change this line:
default_provider = "glm"  # ❌ Wrong - uses Standard API (prepaid balance)

# To this:
default_provider = "zai"  # ✅ Correct - uses Coding Plan (Pro subscription)

# Restart daemon
# Kill existing process, then:
zeroclaw daemon
```

**Key Difference**:
- `"glm"` → `https://api.z.ai/api/paas/v4` (Standard API - needs prepaid balance)
- `"zai"` → `https://api.z.ai/api/coding/paas/v4` (Coding Plan - uses Pro quota)

#### Other API Errors

```bash
# Verify API key is valid
curl -H "Authorization: Bearer your_key" https://open.bigmodel.cn/api/paas/v4/models

# Check provider connection
zeroclaw status | grep provider
```

## Next Steps

1. **Set up your Telegram bot** with BotFather
2. **Get your z.ai API key** from the platform
3. **Copy the config template** to `~/.zeroclaw/config.toml`
4. **Replace placeholders** with your actual tokens and user ID
5. **Create workspace directory**: `mkdir -p ~/zeroclaw-workspace`
6. **Start the daemon**: `zeroclaw daemon`
7. **Test with a message**: Send a message to your bot in Telegram

## Additional Resources

- ZeroClaw Documentation: https://github.com/zeroclaw-labs/zeroclaw
- z.ai Platform: https://open.bigmodel.cn/
- Telegram Bot API: https://core.telegram.org/bots/api
- Config Reference: `docs/config-reference.md`
- Channel Reference: `docs/channels-reference.md`
