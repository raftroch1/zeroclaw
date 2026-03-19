# Lana2.0 - ZeroClaw AI Agent

A Telegram-based AI agent powered by ZeroClaw and z.ai's GLM-4.7 model.

## 🚀 Quick Start

### 1. Build ZeroClaw

```bash
cargo build --release
```

### 2. Configure the Agent

Copy the example config and add your API keys:

```bash
cp config-example.toml ~/.zeroclaw/config.toml
```

Edit `~/.zeroclaw/config.toml` and update:
- `api_key` - Your z.ai API key
- `channels_config.telegram.bot_token` - Your Telegram bot token from @BotFather
- `channels_config.telegram.allowed_users` - Your Telegram user ID

### 3. Create Workspace

```bash
mkdir -p ~/.zeroclaw/workspace
```

### 4. Start the Agent

```bash
./target/release/zeroclaw.exe daemon
```

### 5. Test Your Bot

Send a message to your Telegram bot and start chatting!

## 🤖 Capabilities

- **Model**: GLM-4.7 (200K context, 128K output)
- **Provider**: z.ai Coding Plan (Pro subscription)
- **Tools**: Shell commands, file operations, web search
- **Channels**: Telegram
- **Memory**: SQLite with auto-save

## 📝 Configuration

### Provider Setup

**Important**: Use `zai` provider for Coding Plan (Pro subscription):

```toml
default_provider = "zai"  # ✅ Coding Plan (Pro subscription)
default_model = "glm-4.7"
```

Do NOT use `glm` provider - it requires prepaid balance:

```toml
default_provider = "glm"  # ❌ Standard API (prepaid balance only)
```

### Telegram Setup

1. Create a bot via [@BotFather](https://t.me/BotFather) on Telegram
2. Copy the bot token
3. Get your Telegram user ID (send a message to the bot, it will log your ID)
4. Update the config with both values

### Security Features

- ✅ Workspace-scoped file operations
- ✅ Allowlisted shell commands only
- ✅ User allowlist for Telegram
- ✅ Encrypted API keys at rest
- ✅ Rate limiting (100 actions/hour)

## 🛠️ Available Commands

### Shell Commands

All standard development tools are available within the workspace:
- `ls`, `cat`, `grep`, `find`, `head`, `tail`
- `git`, `cargo`, `rustc`
- `python3`, `node`, `npm`, `pip`
- `curl`, `echo`, `cd`, `pwd`
- And many more...

### File Operations

- Read files: `cat`, `head`, `tail`
- Write files: `echo`, `touch`
- List files: `ls`, `find`
- Manage files: `mkdir`, `rm`, `cp`, `mv`

## 📚 Documentation

- [Setup Instructions](SETUP_INSTRUCTIONS.md)
- [Agent Setup Guide](AGENT_SETUP_GUIDE.md)
- [ZeroClaw Documentation](https://www.zeroclawlabs.ai/docs)
- [OpenClaw z.ai Provider](https://docs.openclaw.ai/providers/zai)

## 🔧 Troubleshooting

### Error 1113: "Insufficient balance"

**Cause**: Using wrong provider endpoint.

**Solution**: Ensure you're using `zai` provider:
```bash
# Check current provider
cat ~/.zeroclaw/config.toml | grep default_provider

# Should be: default_provider = "zai"
```

### Bot Not Responding

```bash
# Check if daemon is running
tasklist | grep zeroclaw

# Check logs
./target/release/zeroclaw.exe status
```

### Telegram Conflicts

If you see "Conflict: terminated by other getUpdates request":
- Kill existing zeroclaw processes
- Ensure only one daemon is running
- Check your bot token isn't used elsewhere

## 📦 Requirements

- Rust (for building ZeroClaw)
- z.ai API key with Pro subscription
- Telegram bot token from @BotFather
- Windows/Linux/macOS

## 🤝 Contributing

This is a personal AI agent setup. For contributing to ZeroClaw:
- Visit: https://github.com/raftroch1/zeroclaw
- Read: [CONTRIBUTING.md](CONTRIBUTING.md)

## 📄 License

This project follows the ZeroClaw license.

## 🙏 Acknowledgments

- [ZeroClaw](https://www.zeroclawlabs.ai/) - Autonomous agent framework
- [z.ai](https://open.bigmodel.cn/) - GLM-4.7 model and API
- [Telegram](https://telegram.org/) - Bot platform
