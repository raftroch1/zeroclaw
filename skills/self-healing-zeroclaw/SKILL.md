---
name: self-healing-zeroclaw
description: Self-healing and diagnostic capabilities for ZeroClaw AI agent. Use when ZeroClaw encounters errors, crashes, tool failures, or needs automated recovery. Handles daemon restarts, configuration validation, service health checks, and automatic issue resolution.
license: Apache-2.0
metadata:
  author: ZeroClaw Community
  version: "1.0.0"
  category: operations
  keywords: [self-healing, diagnostics, recovery, health-check, troubleshooting]
---

# ZeroClaw Self-Healing Skill

Automatic error detection, diagnosis, and recovery for ZeroClaw AI agent runtime.

## Quick Start

When ZeroClaw has issues, this skill provides:

1. **Automatic Detection**: Identify crashes, hangs, and errors
2. **Root Cause Analysis**: Diagnose configuration, API, and system issues
3. **Auto-Recovery**: Restart daemons, fix configs, clear stuck states
4. **Verification**: Confirm services are healthy after recovery

## Core Principles

### Healing First, Ask Questions Later

- **Default to healing**: When errors are detected, attempt recovery immediately
- **Log everything**: All healing actions are logged for audit trails
- **Safe defaults**: Recovery actions use conservative, safe operations
- **Human in the loop**: For destructive operations, ask for confirmation

### Progressive Recovery

Try recovery in order of safety:

1. **Soft recovery** (restart service, clear cache)
2. **Medium recovery** (fix config, restart daemon)
3. **Hard recovery** (kill processes, reset state)
4. **Manual intervention** (alert human operator)

## ZeroClaw Architecture Context

ZeroClaw consists of:

- **Daemon** (`zeroclaw daemon`) - Background service running gateway + channels
- **Gateway** (`127.0.0.1:3000`) - Webhook server for incoming messages
- **Channels** - Telegram, Discord, Slack integrations
- **Agent Loop** - Message processing with tool execution
- **Memory** - SQLite storage for conversations and embeddings
- **Providers** - LLM API connections (NVIDIA NIM, OpenAI, Anthropic, etc.)

### Key Files and Locations

| Component | Location |
|-----------|----------|
| **Binary** | `./target/release/zeroclaw` (project dir) |
| **Config** | `~/.zeroclaw/config.toml` |
| **Environment** | `.env` in project dir (gitignored) |
| **Workspace** | `~/.zeroclaw/workspace/` |
| **Logs** | `/tmp/zeroclaw-daemon.log` (or custom) |
| **Database** | `~/.zeroclaw/workspace/memory/*.db` |

## Common Issues and Healing

### Issue 1: Daemon Not Running

**Symptoms**: Telegram bot doesn't respond, `/tmp/zeroclaw-daemon.log` not updating

**Diagnosis**:
```bash
ps aux | grep "[z]eroclaw daemon"
```

**Healing Steps**:
```bash
# Check for stuck processes
ps aux | grep zeroclaw

# Kill any stuck daemons
pkill -f "zeroclaw daemon" || killall zeroclaw

# Restart with proper environment
cd /path/to/zeoclaw_secure-main
export NVIDIA_API_KEY="your-key-here"
nohup ./target/release/zeroclaw daemon > /tmp/zeroclaw-daemon.log 2>&1 &

# Verify startup
tail -20 /tmp/zeroclaw-daemon.log
```

**Verification**: Daemon should show "Listening for messages..." in logs

---

### Issue 2: Empty Telegram Responses

**Symptoms**: Bot shows "typing" but no response, or empty messages

**Root Cause**: Gateway doesn't support tool execution in current ZeroClaw version (known issue in `src/gateway/mod.rs` line 836)

**Workaround**: Use simple prompts that don't require tools

**Diagnosis**:
```bash
# Check for tool execution errors in logs
grep -i "tool\|error\|failed" /tmp/zeroclaw-daemon.log | tail -20
```

**Healing**:
- **Short term**: Restart daemon and avoid tool-heavy prompts
- **Long term**: Patch gateway to use agent tool execution flow (requires code modification)

---

### Issue 3: API Provider Errors

**Symptoms**: "404 Not Found", "401 Unauthorized", "429 Rate Limit"

**Diagnosis**:
```bash
# Check API key is set
env | grep API_KEY

# Test provider directly
./target/release/zeroclay agent -p nvidia --model z-ai/glm4.7 -m "Test"
```

**Healing Steps**:
```bash
# Verify NVIDIA API key format (starts with nvapi-)
echo $NVIDIA_API_KEY | grep "^nvapi-"

# If key is invalid, update .env file
# Then restart daemon
pkill -f "zeroclaw daemon"
# Restart with new key
```

**Common Provider Models**:
| Provider | Model Name | Notes |
|----------|------------|-------|
| NVIDIA NIM | `z-ai/glm4.7` | Free, works well |
| NVIDIA NIM | `minimaxai/minimax-m2.1` | Free alternative |
| NVIDIA NIM | `moonshotai/kimi-k2-thinking` | Reasoning model |
| GLM (Zhipu) | `glm-5` | Requires paid subscription |

---

### Issue 4: Memory Database Corruption

**Symptoms**: "database is locked", "disk I/O error", memory not saving

**Diagnosis**:
```bash
# Check database files
ls -la ~/.zeroclaw/workspace/memory/

# Test database integrity
sqlite3 ~/.zeroclaw/workspace/memory/*.db "PRAGMA integrity_check;"
```

**Healing Steps**:
```bash
# Backup current memory
cp ~/.zeroclaw/workspace/memory/*.db ~/.zeroclaw/workspace/memory/backup-$(date +%s).db

# Vacuum and rebuild
sqlite3 ~/.zeroclaw/workspace/memory/*.db "VACUUM;"

# If corrupted, start fresh (WARNING: loses memory)
rm ~/.zeroclaw/workspace/memory/*.db
# Restart daemon - will create new database
```

---

### Issue 5: Telegram Bot Not Receiving Messages

**Symptoms**: Messages sent but no "Processing message..." in logs

**Diagnosis**:
```bash
# Check bot token is valid
curl -s "https://api.telegram.org/bot<BOT_TOKEN>/getMe"

# Check for webhook conflicts
curl -s "https://api.telegram.org/bot<BOT_TOKEN>/getWebhookInfo"
```

**Healing Steps**:
```bash
# Clear any conflicting webhooks
curl -s "https://api.telegram.org/bot<BOT_TOKEN>/deleteWebhook"

# Restart daemon to re-establish polling
pkill -f "zeroclaw daemon"
# Restart daemon
```

---

### Issue 6: Out of Memory / High CPU Usage

**Symptoms**: Daemon using >1GB RAM, CPU stuck at 100%

**Diagnosis**:
```bash
# Check resource usage
ps aux | grep zeroclaw

# Check for memory leaks in logs
grep -i "memory\|leak\|oom" /tmp/zeroclaw-daemon.log
```

**Healing Steps**:
```bash
# Graceful restart
pkill -TERM -f "zeroclaw daemon"
sleep 3
# Force kill if still running
pkill -9 -f "zeroclaw daemon"

# Clear any large log files
> /tmp/zeroclaw-daemon.log

# Restart daemon
```

**Prevention**:
- Set `max_in_flight_messages` limit in config
- Enable memory hygiene: `hygiene_enabled = true`
- Regular maintenance: `archive_after_days = 7`

## Health Check Script

Save as `check_zeroclaw_health.sh`:

```bash
#!/bin/bash
echo "=== ZeroClaw Health Check ==="

# 1. Daemon running?
if ps aux | grep -q "[z]eroclaw daemon"; then
    echo "✅ Daemon: Running"
else
    echo "❌ Daemon: Not running"
    exit 1
fi

# 2. Gateway responding?
if curl -s http://127.0.0.1:3000/health > /dev/null; then
    echo "✅ Gateway: Healthy"
else
    echo "❌ Gateway: Not responding"
fi

# 3. API key configured?
if [ -n "$NVIDIA_API_KEY" ]; then
    echo "✅ API Key: Set"
else
    echo "⚠️  API Key: Not set in environment"
fi

# 4. Telegram bot token valid?
BOT_TOKEN="your-bot-token-here"
if curl -s "https://api.telegram.org/bot${BOT_TOKEN}/getMe" | grep -q "true"; then
    echo "✅ Telegram Bot: Valid"
else
    echo "❌ Telegram Bot: Invalid token"
fi

# 5. Memory database OK?
if [ -f ~/.zeroclaw/workspace/memory/*.db ]; then
    echo "✅ Memory Database: Present"
else
    echo "⚠️  Memory Database: Not found (will be created)"
fi

# 6. Log file size
LOG_SIZE=$(du -h /tmp/zeroclaw-daemon.log 2>/dev/null | cut -f1)
echo "📋 Log Size: ${LOG_SIZE:-N/A}"

# 7. Recent errors?
ERROR_COUNT=$(grep -i "error\|failed\|panic" /tmp/zeroclaw-daemon.log 2>/dev/null | wc -l)
if [ "$ERROR_COUNT" -gt 0 ]; then
    echo "⚠️  Recent Errors: $ERROR_COUNT (check logs)"
else
    echo "✅ No Recent Errors"
fi

echo "=== Health Check Complete ==="
```

## Auto-Healing Configuration

Add to `~/.zeroclaw/config.toml` for better resilience:

```toml
[memory]
hygiene_enabled = true              # Auto-cleanup old memories
archive_after_days = 7              # Archive old conversations
purge_after_days = 30               # Delete old data

[autonomy]
level = "supervised"                 # Require approval for dangerous ops
max_actions_per_hour = 20           # Limit runaway actions
max_cost_per_day_cents = 500        # Limit API costs

[reliability]
provider_retries = 3                # Retry failed API calls
provider_backoff_ms = 1000          # Wait between retries
```

## Monitoring Setup

### Systemd Service (Recommended)

Create `/etc/systemd/system/zeroclaw.service`:

```ini
[Unit]
Description=ZeroClaw AI Agent Daemon
After=network.target

[Service]
Type=simple
User=your-username
WorkingDirectory=/path/to/zeroclaw_secure-main
Environment="NVIDIA_API_KEY=your-api-key"
EnvironmentFile=/path/to/.env
ExecStart=/path/to/zeroclaw_secure-main/target/release/zeroclaw daemon
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable zeroclaw
sudo systemctl start zeroclaw
sudo systemctl status zeroclaw
```

### Log Rotation

Create `/etc/logrotate.d/zeroclaw`:

```
/tmp/zeroclaw-daemon.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    copytruncate
}
```

## Emergency Procedures

### Full System Reset

If everything is broken:

```bash
#!/bin/bash
# Complete ZeroClaw reset

# 1. Stop all processes
pkill -9 -f zeroclaw

# 2. Backup current state
BACKUP_DIR="$HOME/zeroclaw-backup-$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"
cp -r ~/.zeroclaw "$BACKUP_DIR/"
cp /tmp/zeroclaw-daemon.log "$BACKUP_DIR/"

# 3. Clear all state
rm -rf ~/.zeroclaw/workspace/state/*
rm -rf ~/.zeroclaw/workspace/memory/*.db*

# 4. Reset config to defaults (optional)
# mv ~/.zeroclaw/config.toml ~/.zeroclaw/config.toml.backup

# 5. Restart
cd /path/to/zeroclaw_secure-main
export NVIDIA_API_KEY="your-key"
./target/release/zeroclaw daemon > /tmp/zeroclaw-daemon.log 2>&1 &

echo "ZeroClaw reset complete. Backup at: $BACKUP_DIR"
```

## Integration with This Skill

When Claude is using this skill:

1. **Always run diagnostics first** before attempting fixes
2. **Use non-destructive methods first** (restart vs delete)
3. **Log all actions** for audit trail
4. **Verify healing was successful** before considering issue resolved
5. **Escalate to human** if issue recurs more than 3 times

## References

- ZeroClaw Documentation: `./docs/` directory
- Troubleshooting Guide: `./docs/troubleshooting.md`
- Operations Runbook: `./docs/operations-runbook.md`
- Configuration Reference: `./docs/config-reference.md`

## Version History

- **1.0.0** (2025-02-28): Initial self-healing skill for ZeroClaw
