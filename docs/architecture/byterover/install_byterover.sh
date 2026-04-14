#!/usr/bin/env bash
set -u

# install_byterover.sh
# Installs and initializes ByteRover inside the ZeroClaw Docker container.
# Safe behavior: daemon failure is logged but does not crash container startup.

LOG_PREFIX="[ByteRover-Install]"
PROJECT_ROOT="${PROJECT_ROOT:-/workspace}"
DAEMON_HOST="${BRV_DAEMON_HOST:-127.0.0.1}"
DAEMON_PORT="${BRV_DAEMON_PORT:-3173}"
DAEMON_LOG="${BRV_DAEMON_LOG:-/var/log/byterover-daemon.log}"

log() {
  echo "${LOG_PREFIX} $*"
}

warn() {
  echo "${LOG_PREFIX} WARN: $*" >&2
}

err() {
  echo "${LOG_PREFIX} ERROR: $*" >&2
}

health_check() {
  # Health check should be non-fatal to preserve graceful degradation.
  if command -v brv >/dev/null 2>&1; then
    if brv status >/dev/null 2>&1; then
      log "Health check OK: brv status succeeded"
      return 0
    fi
    warn "Health check degraded: brv status failed"
    return 1
  else
    warn "Health check degraded: brv binary missing"
    return 1
  fi
}

install_cli() {
  if command -v brv >/dev/null 2>&1; then
    log "brv already installed"
    return 0
  fi

  if ! command -v npm >/dev/null 2>&1; then
    err "npm is required but not found"
    return 1
  fi

  log "Installing ByteRover CLI via npm"
  if npm install -g @campfirein/byterover-cli; then
    log "Installed @campfirein/byterover-cli"
    return 0
  else
    err "Failed to install ByteRover CLI"
    return 1
  fi
}

init_project() {
  if [ ! -d "${PROJECT_ROOT}" ]; then
    warn "PROJECT_ROOT does not exist: ${PROJECT_ROOT}"
    return 1
  fi

  cd "${PROJECT_ROOT}" || return 1

  if [ -f ".brv/config.json" ]; then
    log "Project already initialized at ${PROJECT_ROOT}"
    return 0
  fi

  log "Initializing ByteRover project in ${PROJECT_ROOT}"
  if brv init --yes; then
    log "Project initialized"
    return 0
  else
    warn "Project init failed (continuing in degraded mode)"
    return 1
  fi
}

start_daemon() {
  mkdir -p "$(dirname "${DAEMON_LOG}")"

  # Avoid duplicate daemon starts in simple container workflows.
  if pgrep -f "brv daemon" >/dev/null 2>&1; then
    log "ByteRover daemon already running"
    return 0
  fi

  log "Starting ByteRover daemon on ${DAEMON_HOST}:${DAEMON_PORT}"
  nohup brv daemon --host "${DAEMON_HOST}" --port "${DAEMON_PORT}" >>"${DAEMON_LOG}" 2>&1 &
  sleep 2

  if health_check; then
    log "Daemon started successfully"
    return 0
  else
    warn "Daemon failed to become healthy; continuing without ByteRover"
    return 1
  fi
}

verify_mcp() {
  # Non-blocking verification: test command wiring only.
  if timeout 5s brv mcp --help >/dev/null 2>&1; then
    log "MCP command verified"
    return 0
  fi

  warn "MCP verification failed; ZeroClaw can still run with SQLite + Mem0"
  return 1
}

main() {
  local hard_fail=0

  install_cli || hard_fail=1

  if [ "${hard_fail}" -eq 1 ]; then
    err "CLI installation failed; cannot continue ByteRover setup"
    # Hard fail only installation; caller decides whether to exit.
    return 1
  fi

  init_project || true
  start_daemon || true
  verify_mcp || true
  health_check || true

  log "Setup finished (possibly degraded)."
  return 0
}

# Allow sourcing without executing.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  main "$@"
fi
