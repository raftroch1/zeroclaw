# ZeroClaw – Persistent Docker Deployment Guide

> **Phase 1 Foundation** — eliminates the "fix → restart → revert" loop by
> baking the correct config into the image and bind-mounting runtime state.

---

## Problem

Previously, ZeroClaw's Dockerfile copied `dev/config.template.toml` (an Ollama
dev stub) into the image as the production config. Combined with the lack of
persistent volume mounts, every container restart or rebuild would:

1. Overwrite `config.toml` with Ollama dev defaults
2. Lose any pip-installed packages, skill files, or memory DB changes
3. Revert policy.rs / hermes.rs runtime fixes

## Solution

| Change | What it does |
|--------|-------------|
| **Dockerfile fix** | Now copies `docker-config/config.toml` (production config with correct provider, model routes, MCP servers, Telegram, autonomy policy) into both `dev` and `release` stages |
| **docker-compose.yml** | Bind-mounts `.zeroclaw/` (config, memory, skills) and `workspace/` to the host so runtime changes persist across restarts and rebuilds |
| **`.env` file** | All secrets and tunables live in `.env`, loaded via `env_file:` — never baked into the image |

---

## Quick Start

```bash
# 1. Clone the repo (if you haven't already)
git clone https://github.com/zeroclaw-labs/zeroclaw.git
cd zeroclaw

# 2. Create your .env from the template
cp .env.example .env
# Edit .env — set API_KEY at minimum
nano .env

# 3. Build the image (bakes production config)
docker compose build

# 4. Start with persistent volumes
docker compose up -d

# 5. Verify
docker compose ps
docker compose logs -f zeroclaw
```

The gateway is available at **http://localhost:3000** (or whatever `HOST_PORT`
you set in `.env`).

---

## Directory Layout (Host Side)

After first run, the compose file creates these directories next to
`docker-compose.yml`:

```
zeroclaw/
├── .zeroclaw/              ← bind-mounted to /zeroclaw-data/.zeroclaw/
│   ├── config.toml         ← editable config (persists!)
│   ├── memory.db           ← SQLite memory store
│   ├── skills/             ← learned skills
│   ├── cron/               ← cron job state
│   └── secrets/            ← encrypted secrets store
├── workspace/              ← bind-mounted to /zeroclaw-data/workspace/
│   └── (agent projects)
├── docker-config/
│   └── config.toml         ← source-of-truth config (baked into image)
├── docker-compose.yml
├── .env                    ← your secrets (git-ignored)
└── .env.example
```

You can override these paths via `.env`:

```bash
ZEROCLAW_CONFIG_DIR=/opt/zeroclaw/config
ZEROCLAW_WORKSPACE_DIR=/opt/zeroclaw/workspace
```

---

## How Config Resolution Works

1. **Build time**: `docker-config/config.toml` is `COPY`'d into the image
2. **Runtime**: The bind-mount `.zeroclaw/ → /zeroclaw-data/.zeroclaw/` overlays
   the baked config
3. **First run**: The directory is empty, so the container uses the baked config.
   You can then copy it out and edit:
   ```bash
   # Copy the baked config to your host for editing
   docker compose cp zeroclaw:/zeroclaw-data/.zeroclaw/config.toml ./.zeroclaw/config.toml
   # Edit it
   nano ./.zeroclaw/config.toml
   # Restart to pick up changes
   docker compose restart
   ```
4. **Subsequent runs**: Your host-side config is mounted in and takes effect
   immediately

---

## Common Operations

### Rebuild After Source Changes

```bash
docker compose build --no-cache
docker compose up -d
```

Your `.zeroclaw/` and `workspace/` directories are **untouched** by rebuilds
because they are bind-mounted from the host.

### Rebuild After Config Changes (in docker-config/)

If you edit `docker-config/config.toml` (the source-of-truth in the repo):

```bash
# Option A: Rebuild to bake the new config into the image
docker compose build
docker compose up -d

# Option B: Also copy to your host mount so the running container sees it
cp docker-config/config.toml ./.zeroclaw/config.toml
docker compose restart
```

### Install Python Packages That Persist

Since `/zeroclaw-data/.zeroclaw/` is mounted, you can install into a venv there:

```bash
docker compose exec zeroclaw bash
python3 -m venv /zeroclaw-data/.zeroclaw/venv
source /zeroclaw-data/.zeroclaw/venv/bin/activate
pip install some-package
```

The venv lives on the bind-mounted volume and survives restarts.

### View Logs

```bash
docker compose logs -f zeroclaw
```

### Stop / Start

```bash
docker compose down      # stop & remove container (data persists)
docker compose up -d     # start again with same data
```

### Full Reset (Wipe All Data)

```bash
docker compose down
rm -rf .zeroclaw/ workspace/
docker compose up -d     # fresh start with baked config
```

---

## Port Reference

| Port | Service | `.env` variable |
|------|---------|-----------------|
| 3000 | Gateway API | `HOST_PORT` |
| 8001 | Voice API | `VOICE_PORT` |

---

## Persistence Verification

To confirm changes survive restarts:

```bash
# 1. Make a change inside the container
docker compose exec zeroclaw bash -c 'echo "test=true" >> /zeroclaw-data/.zeroclaw/config.toml'

# 2. Restart
docker compose restart

# 3. Verify the change is still there
docker compose exec zeroclaw bash -c 'tail -1 /zeroclaw-data/.zeroclaw/config.toml'
# Should print: test=true
```

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| Config reverts on restart | `.zeroclaw/` not mounted | Check `volumes:` in compose; ensure `.env` vars are correct |
| Permission denied on mounted dirs | UID mismatch | Run `sudo chown -R 65534:65534 .zeroclaw/ workspace/` on host |
| Container won't start | Missing API_KEY | Set `API_KEY=...` in `.env` |
| Gateway unreachable | Port conflict | Change `HOST_PORT` in `.env` |
