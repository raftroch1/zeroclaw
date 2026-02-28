#!/bin/bash
# =============================================================================
# ZeroClaw Bubblewrap Wrapper
# =============================================================================
# OS-level sandboxing using Bubblewrap (bwrap)
# Alternative to Firejail for lighter-weight sandboxing
#
# Installation:
#   sudo apt install bubblewrap
#
# Usage:
#   ./bubblewrap_wrapper.sh [zeroclaw arguments]
#
# =============================================================================

set -euo pipefail

# Configuration
ZEROCLAW_BIN="/opt/zeroclaw/zeroclaw"
ZEROCLAW_CONFIG="/etc/zeroclaw/config.toml"
WORKSPACE="/var/lib/zeroclaw"
LOG_DIR="/var/log/zeroclaw"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Check if bubblewrap is installed
if ! command -v bwrap &> /dev/null; then
    echo -e "${RED}[ERROR]${NC} Bubblewrap (bwrap) is not installed."
    echo "Install with: sudo apt install bubblewrap"
    exit 1
fi

# Check if zeroclaw binary exists
if [[ ! -f "$ZEROCLAW_BIN" ]]; then
    echo -e "${RED}[ERROR]${NC} ZeroClaw binary not found: $ZEROCLAW_BIN"
    exit 1
fi

# Check if config exists
if [[ ! -f "$ZEROCLAW_CONFIG" ]]; then
    echo -e "${RED}[ERROR]${NC} ZeroClaw config not found: $ZEROCLAW_CONFIG"
    exit 1
fi

echo -e "${GREEN}[INFO]${NC} Starting ZeroClaw in Bubblewrap sandbox..."

# =============================================================================
# Bubblewrap Configuration
# =============================================================================
# This creates a minimal filesystem namespace with only necessary access

exec bwrap \
    \
    `# =========================================================================` \
    `# Namespace Isolation` \
    `# =========================================================================` \
    --unshare-user-try \
    --unshare-ipc \
    --unshare-uts \
    --unshare-cgroup-try \
    `# Note: --unshare-net would disable all networking` \
    `# Use it if ZeroClaw doesn't need network access` \
    `# --unshare-net` \
    \
    `# =========================================================================` \
    `# Filesystem - Minimal Access` \
    `# =========================================================================` \
    `# Create a new root with only necessary bindings` \
    --ro-bind /usr/lib /usr/lib \
    --ro-bind /usr/lib64 /usr/lib64 2>/dev/null || true \
    --ro-bind /lib /lib \
    --ro-bind /lib64 /lib64 2>/dev/null || true \
    \
    `# ZeroClaw binary (read-only)` \
    --ro-bind "$ZEROCLAW_BIN" /opt/zeroclaw/zeroclaw \
    \
    `# Configuration (read-only)` \
    --ro-bind "$ZEROCLAW_CONFIG" /etc/zeroclaw/config.toml \
    --ro-bind /etc/zeroclaw /etc/zeroclaw \
    \
    `# Workspace (read-write)` \
    --bind "$WORKSPACE" /var/lib/zeroclaw \
    \
    `# Log directory (read-write)` \
    --bind "$LOG_DIR" /var/log/zeroclaw \
    \
    `# Essential system files (read-only)` \
    --ro-bind /etc/localtime /etc/localtime \
    --ro-bind /etc/timezone /etc/timezone 2>/dev/null || true \
    --ro-bind /etc/resolv.conf /etc/resolv.conf \
    --ro-bind /etc/hosts /etc/hosts \
    --ro-bind /etc/ssl /etc/ssl \
    --ro-bind /etc/ca-certificates /etc/ca-certificates 2>/dev/null || true \
    \
    `# Minimal /dev` \
    --dev /dev \
    \
    `# Minimal /proc (for basic process info)` \
    --proc /proc \
    \
    `# Private tmp` \
    --tmpfs /tmp \
    \
    `# =========================================================================` \
    `# Block Sensitive Paths (make them inaccessible)` \
    `# =========================================================================` \
    --tmpfs /root \
    --tmpfs /home \
    --tmpfs /boot \
    --tmpfs /mnt \
    --tmpfs /media \
    --tmpfs /srv \
    --tmpfs /run \
    \
    `# =========================================================================` \
    `# Environment Variables` \
    `# =========================================================================` \
    --clearenv \
    --setenv ZEROCLAW_CONFIG /etc/zeroclaw/config.toml \
    --setenv HOME /var/lib/zeroclaw \
    --setenv LANG C.UTF-8 \
    --setenv LC_ALL C.UTF-8 \
    --setenv PATH /usr/local/bin:/usr/bin:/bin \
    --setenv RUST_BACKTRACE 0 \
    \
    `# =========================================================================` \
    `# Process Settings` \
    `# =========================================================================` \
    --new-session \
    --die-with-parent \
    --hostname zeroclaw-sandbox \
    \
    `# =========================================================================` \
    `# Capability Restrictions` \
    `# =========================================================================` \
    --cap-drop ALL \
    \
    `# =========================================================================` \
    `# Seccomp (optional - requires seccomp JSON file)` \
    `# =========================================================================` \
    `# --seccomp 3 3< /etc/zeroclaw/seccomp.json` \
    \
    `# =========================================================================` \
    `# Working Directory` \
    `# =========================================================================` \
    --chdir /var/lib/zeroclaw/workspace \
    \
    `# =========================================================================` \
    `# Execute ZeroClaw` \
    `# =========================================================================` \
    /opt/zeroclaw/zeroclaw --config /etc/zeroclaw/config.toml "$@"
