#!/usr/bin/env bash
set -euo pipefail

log() { echo "[entrypoint] $*"; }
warn() { echo "[entrypoint] WARN: $*" >&2; }

CONFIG_DIR="/zeroclaw-data/.zeroclaw"
CONFIG_PATH="${CONFIG_DIR}/config.toml"
BAKED_CONFIG="/opt/zeroclaw/default-config.toml"
WORKSPACE_DIR="/zeroclaw-data/workspace"
BRV_PROJECT_ROOT="${BRV_PROJECT_ROOT:-${WORKSPACE_DIR}}"

mkdir -p "${CONFIG_DIR}" "${WORKSPACE_DIR}" "${BRV_PROJECT_ROOT}"

# Copy baked config only on first run (volume-friendly).
if [[ ! -f "${CONFIG_PATH}" ]]; then
  if [[ -f "${BAKED_CONFIG}" ]]; then
    log "First boot detected: copying baked config into persistent volume"
    cp "${BAKED_CONFIG}" "${CONFIG_PATH}"
  else
    warn "Baked config not found at ${BAKED_CONFIG}; continuing"
  fi
else
  log "Persistent config already exists; keeping existing config.toml"
fi

# Activate Python virtual environment (pre-baked in image).
if [[ -f "/opt/venv/bin/activate" ]]; then
  # shellcheck disable=SC1091
  source /opt/venv/bin/activate
  log "Activated Python virtual environment at /opt/venv"
else
  warn "Python virtualenv not found at /opt/venv"
fi

# Start ByteRover daemon in background (graceful degradation).
set +e
brv daemon start >/tmp/byterover-daemon.log 2>&1 &
BRV_DAEMON_PID=$!
sleep 2
if brv status >/tmp/byterover-status.log 2>&1; then
  log "ByteRover daemon is running"
else
  warn "ByteRover daemon failed health check; continuing in degraded mode"
fi
set -e

# Initialize ByteRover project knowledge store if missing.
if [[ ! -f "${BRV_PROJECT_ROOT}/.brv/config.json" ]]; then
  log "Initializing ByteRover project at ${BRV_PROJECT_ROOT}"
  set +e
  (cd "${BRV_PROJECT_ROOT}" && brv init --yes) >/tmp/byterover-init.log 2>&1
  if [[ $? -ne 0 ]]; then
    warn "ByteRover project init failed; continuing"
  fi
  set -e
else
  log "ByteRover project already initialized"
fi

cd /zeroclaw-data

# Start ZeroClaw agent runtime.
if [[ $# -eq 0 ]]; then
  log "Starting ZeroClaw daemon"
  exec zeroclaw daemon --host 0.0.0.0 --port "${ZEROCLAW_GATEWAY_PORT:-3000}"
else
  log "Starting ZeroClaw with custom command: $*"
  exec zeroclaw "$@"
fi
