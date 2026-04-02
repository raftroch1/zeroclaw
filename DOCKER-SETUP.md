# Lana Docker Setup Guide

## Why Docker?

**Problem with Native Windows:**
- Environment variables don't propagate correctly to background processes
- Winsock corruption issues (WinError 10106)
- Python networking problems
- Complex workarounds needed for simple things

**Docker Solution:**
- Clean Linux environment
- All environment variables work correctly
- No Windows-specific issues
- Easy configuration and management

## Quick Start

### 1. Start Docker Desktop
Make sure Docker Desktop is running on Windows

### 2. Start Lana
Double-click: `START-LANA-DOCKER.bat`

Or run manually:
```bash
docker compose -f docker-compose.lana.yml up -d --build
```

### 3. View Logs
```bash
docker compose -f docker-compose.lana.yml logs -f
```

### 4. Stop Lana
```bash
docker compose -f docker-compose.lana.yml down
```

## Configuration

All configuration is in:
- `docker-compose.lana.yml` - Container settings
- `C:\Users\rafae\.zeroclaw\config.toml` - ZeroClaw config (mounted as volume)

### Current Setup:
- **Text Model**: glm-5-turbo (Z.AI API)
- **Vision**: zai_vision_analyze tool (GLM-4.6V)
- **Channel**: Telegram
- **MCP Bridge**: Accessible via host network

## What Changed from Windows Native

### Removed (Windows workarounds):
1. ❌ Permanent user environment variables (GOOGLE_API_KEY, GEMINI_API_KEY)
2. ❌ Startup batch scripts with env var hacks
3. ❌ PowerShell startup scripts
4. ❌ Google Gemini automatic routing (was causing API key issues)

### Now (Docker approach):
1. ✅ All environment variables in docker-compose.yml
2. ✅ Simple batch file to start/stop
3. ✅ Clean Linux environment
4. ✅ Z.AI vision tool (no routing needed)

## Vision How It Works Now

**Before (broken):**
- Detect image → Route to Google Gemini → API key fails ❌

**Now (working):**
- Send image to glm-5-turbo → Model calls zai_vision_analyze tool → Vision works ✅

## Troubleshooting

### Container won't start:
```bash
docker compose -f docker-compose.lana.yml logs
```

### Restart container:
```bash
docker compose -f docker-compose.lana.yml restart
```

### Rebuild after code changes:
```bash
docker compose -f docker-compose.lana.yml up -d --build
```

### Access container shell:
```bash
docker exec -it lana-zeroclaw sh
```

## Auto-Start

The container is configured with `restart: unless-stopped`, so it will:
- Start automatically when Docker Desktop starts
- Restart if it crashes
- Keep running until you stop it manually

To make Docker Desktop start on Windows boot:
1. Open Docker Desktop
2. Settings → General
3. Enable "Start Docker Desktop when you sign in to Windows"
