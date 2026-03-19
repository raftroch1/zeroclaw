# ZeroClaw Agent Setup Instructions

## Quick Setup

### 1. Copy Configuration File

```bash
# Copy the production config to ZeroClaw config directory
cp config-example.toml ~/.zeroclaw/config.toml
```

### 2. Create Workspace Directory

```bash
# Create a workspace for the agent to operate in
mkdir -p ~/zeroclaw-workspace
cd ~/zeroclaw-workspace
```

### 3. Verify Configuration

```bash
# Check that config is loaded correctly
zeroclaw status

# You should see:
# - Provider: glm
# - Model: glm-4
# - Telegram: enabled
# - Workspace: ~/zeroclaw-workspace
```

### 4. Start the Agent

```bash
# Start the daemon (recommended for Telegram bot)
zeroclaw daemon

# Or run in foreground for debugging
zeroclaw daemon --foreground
```

### 5. Test the Bot

Send a message to your Telegram bot:
- **Bot**: The bot you created via @BotFather
- **Test message**: "Hello, what can you do?"

The bot should respond with its capabilities.

---

## Configuration Summary

Your agent is configured with:

| Component | Value |
|-----------|--------|
| **Provider** | z.ai (Coding Plan - Pro subscription) ✓ |
| **Model** | glm-4.7 (200K context, 128K output) ✓ |
| **API Endpoint** | Coding Plan (uses Pro quota) ✓ |
| **Telegram Bot Token** | Configured ✓ |
| **Telegram User ID** | 521797087 (allowlisted) ✓ |
| **Workspace** | ~/zeroclaw-workspace (scoped) ✓ |
| **Shell Access** | Allowlisted commands ✓ |
| **File Operations** | Workspace-scoped ✓ |
| **Web Search** | z.ai MCP (Brave) ✓ |
| **Security** | Encrypted secrets ✓ |

---

## Available Tools

The agent has access to the following tools (all workspace-scoped):

### Shell Commands
All standard development and file operations:
- `ls`, `cat`, `grep`, `find`, `head`, `tail`
- `mkdir`, `rm`, `cp`, `mv`, `touch`
- `git`, `cargo`, `rustc`
- `python3`, `node`, `npm`, `pip`
- `curl`, `echo`, `cd`, `pwd`
- Text editors: `vi`, `vim`, `nano`
- File utilities: `tar`, `gzip`, `zip`, `sort`, `uniq`

### File Operations
- **Read files**: `cat`, `head`, `tail`
- **Write files**: `echo`, `touch`
- **List files**: `ls`, `find`
- **Manage files**: `mkdir`, `rm`, `cp`, `mv`

### HTTP/Web Tools
- **HTTP requests**: `curl` (via shell tool)
- **Web search**: z.ai MCP integration
  - Brave search (requires API key)
  - DuckDuckGo search (free)

### Memory
- **Hybrid search**: SQLite with vector + keyword search
- **Auto-save**: Conversations saved automatically
- **Context recall**: Agent can recall past conversations

---

## Security Features

### Multi-Layer Security

1. **Telegram Security**
   - ✅ Allowlist: Only user ID 521797087 can interact
   - ✅ Empty allowlist = deny all (fail-safe)
   - ✅ Bot token never logged or exposed

2. **Workspace Isolation**
   - ✅ All file operations restricted to ~/zeroclaw-workspace
   - ✅ Cannot access /etc, /root, /proc, /sys, etc.
   - ✅ Path traversal attacks blocked
   - ✅ Symlink escape detection enabled

3. **Command Safety**
   - ✅ Only allowlisted commands can execute
   - ✅ Medium-risk commands require approval
   - ✅ High-risk commands blocked (e.g., rm -rf /, sudo)
   - ✅ Rate limiting: 100 actions per hour

4. **Secret Management**
   - ✅ API keys encrypted at rest
   - ✅ Secret key stored separately with restricted permissions
   - ✅ Never logged to output

---

## Common Commands

### Agent Management

```bash
# Start daemon
zeroclaw daemon

# Stop daemon
zeroclaw daemon stop

# Restart daemon
zeroclaw daemon restart

# Check status
zeroclaw status

# View logs
journalctl -u zeroclaw -f

# Check channel health
zeroclaw channel doctor

# Test Telegram connection
zeroclaw channel test telegram
```

### Testing the Agent

```bash
# Send a test message via Telegram
# Message: "List files in workspace"

# Expected response: The agent will use the shell tool
# to run `ls -la` in ~/zeroclaw-workspace

# Example test: Create a file
# Message: "Create a file named test.txt with content 'Hello World'"
# Expected: The agent will use echo and redirect tools
```

---

## Troubleshooting

### Bot Not Responding

```bash
# Check if daemon is running
ps aux | grep zeroclaw

# Check status
zeroclaw status

# View logs for errors
journalctl -u zeroclaw -n 50 | tail -f

# Verify config was loaded
cat ~/.zeroclaw/config.toml | grep api_key
```

### Connection Errors

```bash
# Test Telegram bot API connection
zeroclaw channel doctor

# Check bot token is correct
# Should match: 8694658163:AAGGqREUMKLWnsMvMrvLVJetLa3TpkhS5zA

# Check user ID is in allowlist
cat ~/.zeroclaw/config.toml | grep 521797087
```

### Tools Not Working

```bash
# Check autonomy settings
zeroclaw status | grep -A 20 autonomy

# Verify workspace directory
ls -la ~/zeroclaw-workspace

# Test shell access manually
cd ~/zeroclaw-workspace
ls -la
```

### z.ai API Errors

```bash
# Error 1113: "Insufficient balance or no resource package"
# Cause: Using "glm" provider (Standard API) instead of "zai" (Coding Plan)
# Solution: Use "zai" provider for Pro subscription

# Check current provider
cat ~/.zeroclaw/config.toml | grep default_provider

# Should be: default_provider = "zai" (NOT "glm")
# "zai" uses Coding Plan endpoint (works with Pro subscription)
# "glm" uses Standard API endpoint (requires prepaid balance)

# Test API key validity
curl -H "Authorization: Bearer 323ebd135f2149018d03fe9a92976a91.6LQJQLRy4prgMzLi" \
  https://open.bigmodel.cn/api/paas/v4/models

# Check provider connection
zeroclaw status | grep -A 5 provider
```

---

## Configuration Location

- **Config file**: `~/.zeroclaw/config.toml`
- **Workspace**: `~/zeroclaw-workspace`
- **Logs**: `journalctl -u zeroclaw` (or `~/.local/state/zeroclaw/zeroclaw.log`)
- **Secret key**: `~/.zeroclaw/.secret_key` (encrypted)

---

## Next Steps

1. ✅ **Configuration created**: Copy `config-example.toml` to `~/.zeroclaw/config.toml`
2. ⏳ **Create workspace**: `mkdir -p ~/zeroclaw-workspace`
3. ⏳ **Start daemon**: `zeroclaw daemon`
4. ⏳ **Test bot**: Send a message to your Telegram bot
5. ⏳ **Verify tools**: Ask the agent to run a command like `ls -la`

---

## Support

- **ZeroClaw Docs**: https://github.com/zeroclaw-labs/zeroclaw
- **z.ai Platform**: https://open.bigmodel.cn/
- **Telegram Bot API**: https://core.telegram.org/bots/api
- **Setup Guide**: `AGENT_SETUP_GUIDE.md` (for reference)
