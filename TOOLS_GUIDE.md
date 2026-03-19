# Lana2.0 - Available Tools Guide

Complete list of tools available to Lana in your ZeroClaw agent setup.

## ✅ Currently Enabled Tools

### Shell Commands (Workspace-Scoped)
All commands run within `~/.zeroclaw/workspace/` for security.

**File Operations:**
- `ls` - List directory contents
- `cat` - Read file contents
- `head` - Show first lines of file
- `tail` - Show last lines of file
- `grep` - Search text in files
- `find` - Search for files
- `mkdir` - Create directories
- `rm` - Remove files/directories
- `cp` - Copy files
- `mv` - Move/rename files
- `touch` - Create empty files
- `wc` - Count lines/words/characters
- `sort` - Sort lines
- `uniq` - Remove duplicate lines

**Development Tools:**
- `git` - Version control
- `cargo` - Rust package manager
- `rustc` - Rust compiler
- `python3` - Python 3 interpreter
- `pip` - Python package installer
- `node` - Node.js runtime
- `npm` - Node package manager

**Text Editors:**
- `vi` / `vim` - Vim editor
- `nano` - Nano editor

**System Tools:**
- `curl` - HTTP client (for web requests)
- `echo` - Display text
- `cd` - Change directory
- `pwd` - Print working directory
- `sh` / `bash` - Shell interpreters
- `chmod` - Change file permissions
- `chown` - Change file ownership
- `tar` - Archive files
- `gzip` / `gunzip` - Com/decompress files
- `zip` / `unzip` - Zip archives
- `df` - Disk space
- `du` - Directory size
- `free` - Memory usage
- `top` - Process monitor
- `ps` - Process list
- `kill` / `pkill` / `pgrep` - Process management
- `xargs` - Execute commands from stdin
- `tee` - Read stdin and write to files
- `less` / `more` - Page through text
- `sed` / `awk` - Text processing

### File Read/Write Tools

**Read Operations:**
```python
# Lana can read files
content = read_file("path/to/file.txt")

# Via shell
cat file.txt
head -n 10 file.txt
grep "pattern" file.txt
```

**Write Operations:**
```python
# Lana can write files
write_file("output.txt", "content here")

# Via shell
echo "content" > file.txt
cat > file.txt << EOF
multi-line content
EOF
```

### Python Scripting

Lana can execute Python scripts:

```python
# Data analysis
import pandas as pd
data = pd.read_csv("data.csv")
print(data.describe())

# Web scraping
import requests
response = requests.get("https://api.example.com/data")
print(response.json())

# File processing
with open("input.txt") as f:
    content = f.read()
# Process content...
with open("output.txt", "w") as f:
    f.write(result)
```

### HTTP/Network Tools

**curl for Web Requests:**
```bash
# GET requests
curl https://api.example.com/data

# POST with JSON
curl -X POST http://localhost:8000/api/trade \
  -H "Content-Type: application/json" \
  -d '{"symbol":"SOL-PERP","side":"LONG"}'

# Download files
curl -O https://example.com/file.zip

# Check headers
curl -I https://example.com
```

### Trading Backend Integration

**When trading backend is running:**
```bash
# Execute trade
curl -X POST http://localhost:8000/api/autotrader/test-trade \
  -H "Content-Type: application/json" \
  -d '{
    "symbol": "SOL-PERP",
    "side": "LONG",
    "size": 0.01,
    "leverage": 1.0
  }'

# Check status
curl http://localhost:8000/api/autotrader/status

# View positions
curl http://localhost:8000/api/autotrader/positions
```

### Voice API Integration

**Send voice messages:**
```bash
curl -X POST http://localhost:8001/voice \
  -H "Content-Type: application/json" \
  -d '{"text": "Trade executed successfully!"}'
```

## 🔒 Security Features

### Workspace Isolation
- **All operations** confined to `~/.zeroclaw/workspace/`
- **Cannot access** system directories (/etc, /root, etc.)
- **Cannot access** user home outside workspace
- **Symlink escape** detection enabled

### Command Safety
- **Allowlist only**: Only commands in the list can run
- **Supervised mode**: Medium-risk commands require approval
- **High-risk blocking**: Dangerous commands blocked entirely
- **Rate limiting**: 100 actions per hour maximum

### Risk Management
- **Max positions**: 2 simultaneous trades
- **Stop loss**: 15% max per trade
- **Position size**: 0.01-0.1 SOL max
- **Leverage**: 1.0-2.0x max
- **Daily trades**: 4 maximum

## 🎯 Usage Examples

### Example 1: Read and Analyze File
```
You: "Read the trading log and summarize today's trades"
Lana:
1. Reads: cat ~/.zeroclaw/workspace/trading.log
2. Analyzes with Python: pandas, statistics
3. Summarizes findings
4. Creates summary file if requested
```

### Example 2: Web Request & Analysis
```
You: "Check SOL price on Drift and analyze"
Lana:
1. Fetches: curl https://api.drift.protocol.com/price/SOL
2. Parses JSON response
3. Calculates indicators
4. Provides analysis
```

### Example 3: Execute Trade
```
You: "Open a LONG position on SOL-PERP"
Lana:
1. Checks current positions
2. Analyzes market conditions
3. Confirms risk parameters
4. Executes: curl -X POST http://localhost:8000/api/autotrader/test-trade
5. Sends voice confirmation
6. Monitors position
```

### Example 4: Python Data Processing
```
You: "Create a chart of my trading performance"
Lana:
1. Reads trading data from brain.db
2. Writes Python script using matplotlib
3. Executes: python3 chart.py
4. Sends chart image via Telegram
```

### Example 5: File Operations
```
You: "Organize my trading logs by date"
Lana:
1. Lists files: ls -la ~/.zeroclaw/workspace/logs/
2. Creates directories: mkdir logs/2026-03 logs/2026-04
3. Moves files: mv logs/trade*.log logs/2026-03/
4. Creates index file
```

## 🚨 Important Notes

### What Lana CANNOT Do
- ❌ Access files outside workspace
- ❌ Run commands not in allowlist
- ❌ Execute system-level commands (sudo, etc.)
- ❌ Exceed risk management limits
- ❌ Override safety checks

### Best Practices
1. **Always work in workspace**: Use `~/.zeroclaw/workspace/` for files
2. **Check before trading**: Lana will explain reasoning first
3. **Monitor positions**: Regular status checks on open trades
4. **Use Python for complex tasks**: Better than shell for data processing
5. **Leverage curl**: For API calls and web requests

### Error Handling
If Lana encounters errors:
1. **Shell command fails**: She'll try alternative approaches
2. **File not found**: She'll check path and permissions
3. **API errors**: She'll retry with different parameters
4. **Tool limits**: She'll explain what's needed

## 📝 Tool Configuration

To modify allowed commands, edit `~/.zeroclaw/config.toml`:

```toml
[autonomy]
level = "supervised"
workspace_only = true

allowed_commands = [
    # Add your commands here
    "ls", "cat", "curl", "python3", ...
]

# Risk limits
max_actions_per_hour = 100
max_cost_per_day_cents = 1000
```

---

**Lana2.0 - Your AI trading companion with powerful tools** ☕💜

*Workspace-scoped, security-first, trading-focused*
