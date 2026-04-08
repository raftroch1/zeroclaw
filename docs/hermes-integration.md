# Hermes Agent Integration

ZeroClaw can delegate tasks to [Hermes Agent](https://github.com/lana-z/hermes-agent), a Python-based multi-agent orchestration system, via an HTTP Gateway bridge.

## Architecture

```
┌───────────────────┐    HTTP/REST     ┌──────────────────┐
│ ZeroClaw           │───────────────→ │ Hermes Gateway    │
│ (HermesGatewayTool)│                 │ (hermes gateway)  │
│                    │←───────────────│ port 8080         │
└───────────────────┘   JSON response  └──────────────────┘
        │                                      │
        └──────── shared workspace ────────────┘
                  (Docker volume)
```

**Why HTTP Gateway?**
- No Docker-in-Docker complexity
- Clean process isolation between Rust and Python runtimes
- ZeroClaw already supports HTTP tools
- Hermes gateway is production-ready
- Config persists naturally in Hermes's own volume

## Quick Start

### 1. Enable in `config.toml`

```toml
[hermes]
enabled = true
gateway_url = "http://hermes:8080"   # Docker service name
timeout_secs = 300                   # 5 min for complex tasks
# api_key = ""                       # Optional auth token
# default_profile = ""               # Hermes --profile name
# shared_workspace = "/workspace"    # Shared volume path
```

### 2. Set up Docker Compose

Uncomment the `hermes` service in `docker-compose.yml`:

```yaml
services:
  hermes:
    build:
      context: ../hermes-agent    # Path to Hermes repo
      dockerfile: Dockerfile
    container_name: hermes
    restart: unless-stopped
    command: ["hermes", "gateway", "--port", "8080"]
    environment:
      - HERMES_HOME=/opt/data
    volumes:
      - ./hermes-data:/opt/data        # Persistent state
      - ./workspace:/workspace         # Shared with ZeroClaw
    ports:
      - "8080:8080"
    networks:
      - zeroclaw-net
```

### 3. Configure `.env`

```bash
HERMES_BUILD_CONTEXT=../hermes-agent
HERMES_DATA_DIR=./hermes-data
HERMES_PORT=8080
```

### 4. Start Both Services

```bash
docker compose up -d
```

## Tool Usage

The `hermes_gateway` tool exposes three operations to the LLM:

### Chat

Send a natural language prompt to Hermes:

```json
{
  "operation": "chat",
  "prompt": "Research the latest Rust async patterns and write a summary",
  "model": "anthropic/claude-sonnet-4-20250514",
  "provider": "openrouter"
}
```

### Delegate Task

Delegate a focused goal with specific toolsets:

```json
{
  "operation": "delegate_task",
  "goal": "Fix the authentication bug in auth.py",
  "context": "Error occurs on line 42 when JWT token expires",
  "toolsets": ["terminal", "file"],
  "max_iterations": 50
}
```

Available toolsets: `terminal`, `file`, `web`

### Health Check

Verify Hermes gateway connectivity:

```json
{
  "operation": "health_check"
}
```

## Configuration Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `false` | Enable/disable Hermes integration |
| `gateway_url` | string | `"http://hermes:8080"` | Hermes gateway URL |
| `timeout_secs` | u64 | `300` | Request timeout (seconds) |
| `api_key` | string | `""` | Optional auth token |
| `default_profile` | string | `""` | Hermes `--profile` name |
| `shared_workspace` | string | `""` | Shared volume mount path |

## Hermes Configuration

On the Hermes side, configure `config.yaml` in `HERMES_HOME`:

```yaml
delegation:
  max_iterations: 50
  default_toolsets: ["terminal", "file", "web"]
  # model: "anthropic/claude-sonnet-4-20250514"
  # provider: "openrouter"
```

Key Hermes concepts:
- **HERMES_HOME**: Environment variable pointing to config directory (default: `~/.hermes`)
- **--profile**: CLI flag to select named profiles (`~/.hermes/profiles/<name>`)
- **delegate_task()**: Creates child agents with focused goals and restricted toolsets
- **Max depth**: 2 levels (parent → child → grandchild rejected)
- **Max concurrent**: 3 children per parent

## Volume Mounts

For file exchange between ZeroClaw and Hermes:

```yaml
volumes:
  # ZeroClaw workspace
  - ./workspace:/zeroclaw-data/workspace  # ZeroClaw reads/writes here
  
  # Same directory mounted in Hermes
  - ./workspace:/workspace                # Hermes reads/writes here
```

Both agents can create, read, and modify files in the shared workspace. The `shared_workspace` config option tells Hermes which directory to use as the working directory for delegated tasks.

## Networking

Both containers must be on the same Docker network:

```yaml
networks:
  zeroclaw-net:
    driver: bridge
```

ZeroClaw references Hermes by Docker service name (`http://hermes:8080`).

## Troubleshooting

### Cannot connect to Hermes

1. Check Hermes is running: `docker compose ps hermes`
2. Check logs: `docker compose logs hermes`
3. Verify network: `docker compose exec zeroclaw curl http://hermes:8080/health`
4. Ensure both services are on `zeroclaw-net`

### Timeout errors

- Increase `timeout_secs` in `[hermes]` config (default: 300s)
- Break complex tasks into smaller delegations
- Check Hermes resource limits in docker-compose

### Authentication errors

- Set `api_key` in `[hermes]` config if Hermes requires auth
- Check Hermes gateway configuration for auth requirements

## Files Changed

| File | Purpose |
|------|---------|
| `src/tools/hermes_gateway.rs` | HermesGatewayTool implementation |
| `src/tools/mod.rs` | Tool registration |
| `src/config/schema.rs` | HermesConfig struct |
| `src/config/mod.rs` | Config re-export |
| `docker-compose.yml` | Hermes service definition |
| `.env.example` | Hermes environment variables |
