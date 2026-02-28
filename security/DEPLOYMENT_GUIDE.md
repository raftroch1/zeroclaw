# ZeroClaw Hardened Deployment Guide

This guide provides comprehensive instructions for deploying ZeroClaw with security-hardened configuration on Ubuntu.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Quick Start](#quick-start)
3. [Step-by-Step Installation](#step-by-step-installation)
4. [Configuration Explained](#configuration-explained)
5. [Security Verification](#security-verification)
6. [Systemd Service Setup](#systemd-service-setup)
7. [OS-Level Sandboxing](#os-level-sandboxing)
8. [Troubleshooting](#troubleshooting)
9. [Maintenance](#maintenance)
10. [Performance Tuning](#performance-tuning)

---

## Prerequisites

### System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| OS | Ubuntu 20.04 LTS | Ubuntu 22.04 LTS |
| RAM | 512 MB | 1 GB+ |
| Disk | 1 GB free | 5 GB+ free |
| CPU | 1 core | 2+ cores |

### Required Software

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install dependencies
sudo apt install -y \
    curl \
    tar \
    openssl \
    git

# Optional: For Rust installation (if building from source)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Optional (Recommended) Software

```bash
# OS-level sandboxing
sudo apt install -y firejail    # Option 1
sudo apt install -y bubblewrap  # Option 2 (lighter weight)
```

---

## Quick Start

```bash
# 1. Clone or copy this package
cd ~/zeroclaw_hardened_config

# 2. Make scripts executable
chmod +x setup.sh security_verify.sh sandboxing/*.sh

# 3. Run setup (as root)
sudo ./setup.sh

# 4. Verify security
sudo ./security_verify.sh

# 5. Start service
sudo systemctl enable zeroclaw
sudo systemctl start zeroclaw
```

---

## Step-by-Step Installation

### Step 1: Prepare the System

```bash
# Create dedicated user for ZeroClaw
sudo useradd --system --shell /usr/sbin/nologin --home-dir /var/lib/zeroclaw zeroclaw
```

### Step 2: Create Directory Structure

```bash
# Create directories
sudo mkdir -p /opt/zeroclaw
sudo mkdir -p /etc/zeroclaw
sudo mkdir -p /var/lib/zeroclaw/{workspace,keys,backups}
sudo mkdir -p /var/log/zeroclaw

# Set ownership
sudo chown -R zeroclaw:zeroclaw /var/lib/zeroclaw
sudo chown -R zeroclaw:zeroclaw /var/log/zeroclaw

# Set permissions
sudo chmod 750 /var/lib/zeroclaw
sudo chmod 700 /var/lib/zeroclaw/keys
sudo chmod 750 /var/lib/zeroclaw/workspace
sudo chmod 750 /var/log/zeroclaw
```

### Step 3: Install ZeroClaw Binary

**Option A: Build from Source**
```bash
# Clone repository (adjust URL as needed)
git clone https://github.com/zeroclaw/zeroclaw.git /tmp/zeroclaw
cd /tmp/zeroclaw

# Build release binary
cargo build --release

# Install binary
sudo cp target/release/zeroclaw /opt/zeroclaw/
sudo chmod 755 /opt/zeroclaw/zeroclaw
```

**Option B: Pre-built Binary**
```bash
# Download binary (adjust URL)
curl -L https://releases.zeroclaw.dev/latest/zeroclaw-linux-x64 -o /tmp/zeroclaw
sudo mv /tmp/zeroclaw /opt/zeroclaw/zeroclaw
sudo chmod 755 /opt/zeroclaw/zeroclaw
```

### Step 4: Install Configuration

```bash
# Copy hardened config
sudo cp hardened_config.toml /etc/zeroclaw/config.toml
sudo chown root:zeroclaw /etc/zeroclaw/config.toml
sudo chmod 640 /etc/zeroclaw/config.toml
```

### Step 5: Generate Encryption Keys

```bash
# Master encryption key (256-bit)
sudo openssl rand -base64 32 | sudo tee /var/lib/zeroclaw/keys/master.key > /dev/null
sudo chmod 600 /var/lib/zeroclaw/keys/master.key
sudo chown zeroclaw:zeroclaw /var/lib/zeroclaw/keys/master.key

# API key
sudo openssl rand -hex 32 | sudo tee /var/lib/zeroclaw/keys/api.key > /dev/null
sudo chmod 600 /var/lib/zeroclaw/keys/api.key
sudo chown zeroclaw:zeroclaw /var/lib/zeroclaw/keys/api.key

# IMPORTANT: Back up these keys securely!
echo "Master key: $(sudo cat /var/lib/zeroclaw/keys/master.key)"
echo "API key: $(sudo cat /var/lib/zeroclaw/keys/api.key)"
```

### Step 6: Install Systemd Service

```bash
sudo cp systemd/zeroclaw-hardened.service /etc/systemd/system/zeroclaw.service
sudo systemctl daemon-reload
```

---

## Configuration Explained

### Critical Security Settings

| Setting | Value | Why It Matters |
|---------|-------|----------------|
| `bind_address` | `127.0.0.1` | Prevents remote access |
| `workspace_only` | `true` | Restricts filesystem access |
| `use_allowlist` | `true` | Limits executable commands |
| `skillforge.enabled` | `false` | Prevents loading untrusted code |
| `level` (autonomy) | `supervised` | Requires approval for destructive ops |
| `allow_outbound_requests` | `false` | Prevents data exfiltration |

### Forbidden Paths

The configuration blocks access to:
- `/etc` - System configuration
- `/root`, `/home` - User directories
- `/var/log`, `/var/run` - System state
- `/dev`, `/proc`, `/sys` - Kernel interfaces
- `~/.ssh`, `~/.gnupg`, `~/.aws` - Credentials

### Allowed Commands

Only these categories are allowed:
- **Git**: `git`
- **File inspection**: `ls`, `cat`, `head`, `tail`, `grep`, `find`
- **Development**: `npm`, `cargo`, `python3`, `node`, `go`
- **Text processing**: `echo`, `cut`, `tr`, `jq`

See `allowed_commands.txt` for the complete list with explanations.

---

## Security Verification

### Run Security Check

```bash
sudo ./security_verify.sh
```

### Expected Output

A passing installation shows:
- All file permissions correct
- Localhost-only binding
- Workspace-only mode enabled
- Command allowlist enabled
- SkillForge disabled
- Audit logging enabled

### Manual Verification

```bash
# Check file permissions
ls -la /var/lib/zeroclaw/keys/
# Expected: -rw------- zeroclaw zeroclaw master.key

# Check network binding (when running)
ss -tlnp | grep zeroclaw
# Expected: 127.0.0.1:8080 (NOT 0.0.0.0)

# Check running user
ps aux | grep zeroclaw
# Expected: zeroclaw user (NOT root)
```

---

## Systemd Service Setup

### Enable and Start

```bash
# Enable auto-start on boot
sudo systemctl enable zeroclaw

# Start the service
sudo systemctl start zeroclaw

# Check status
sudo systemctl status zeroclaw
```

### View Logs

```bash
# Real-time logs
sudo journalctl -u zeroclaw -f

# Last 100 lines
sudo journalctl -u zeroclaw -n 100

# Audit logs
sudo tail -f /var/log/zeroclaw/audit.log
```

### Check Systemd Security Score

```bash
sudo systemd-analyze security zeroclaw
# Target: Score < 3.0 (SAFE)
```

### Service Commands

```bash
sudo systemctl stop zeroclaw      # Stop
sudo systemctl restart zeroclaw   # Restart
sudo systemctl reload zeroclaw    # Reload config
```

---

## OS-Level Sandboxing

### Option 1: Firejail (Recommended)

Firejail provides comprehensive sandboxing with easy configuration.

```bash
# Install
sudo apt install firejail

# Install profile
sudo cp sandboxing/firejail_profile.profile /etc/firejail/zeroclaw.profile

# Run with Firejail
firejail --profile=/etc/firejail/zeroclaw.profile /opt/zeroclaw/zeroclaw
```

**To use Firejail with systemd:**

Edit `/etc/systemd/system/zeroclaw.service`:
```ini
ExecStart=/usr/bin/firejail --profile=/etc/firejail/zeroclaw.profile /opt/zeroclaw/zeroclaw --config /etc/zeroclaw/config.toml
```

### Option 2: Bubblewrap (Lightweight)

Bubblewrap is lighter-weight and available in most distributions.

```bash
# Install
sudo apt install bubblewrap

# Make wrapper executable
chmod +x sandboxing/bubblewrap_wrapper.sh

# Run with Bubblewrap
./sandboxing/bubblewrap_wrapper.sh
```

**To use Bubblewrap with systemd:**

Edit `/etc/systemd/system/zeroclaw.service`:
```ini
ExecStart=/path/to/bubblewrap_wrapper.sh
```

### Choosing Between Firejail and Bubblewrap

| Feature | Firejail | Bubblewrap |
|---------|----------|------------|
| Ease of use | ★★★★★ | ★★★☆☆ |
| Configuration | Profile files | Command line |
| Overhead | Low | Very low |
| Features | Many | Minimal |
| Best for | General use | Minimal footprint |

---

## Troubleshooting

### Service Won't Start

```bash
# Check logs
sudo journalctl -u zeroclaw -n 50

# Verify binary exists
ls -la /opt/zeroclaw/zeroclaw

# Verify config syntax
cat /etc/zeroclaw/config.toml | head -50

# Test manual start
sudo -u zeroclaw /opt/zeroclaw/zeroclaw --config /etc/zeroclaw/config.toml
```

### Permission Denied Errors

```bash
# Re-apply permissions
sudo chown -R zeroclaw:zeroclaw /var/lib/zeroclaw
sudo chown -R zeroclaw:zeroclaw /var/log/zeroclaw
sudo chmod 700 /var/lib/zeroclaw/keys
```

### Can't Access from Network

**This is intentional.** The hardened configuration only allows localhost access.

If you need network access (not recommended):
1. Change `bind_address` to `0.0.0.0` (security risk!)
2. Enable TLS in configuration
3. Configure firewall rules
4. Consider a reverse proxy with authentication

### Commands Being Blocked

```bash
# Check allowed commands
grep 'allowed_commands' /etc/zeroclaw/config.toml

# Add needed commands to the allowlist (carefully!)
sudo nano /etc/zeroclaw/config.toml
```

### Memory Issues on Old Hardware

```bash
# Reduce memory limits in config
sudo nano /etc/zeroclaw/config.toml
# Change: max_memory_mb = 256
# Change: max_context_tokens = 4096
```

---

## Maintenance

### Updating ZeroClaw

```bash
# Stop service
sudo systemctl stop zeroclaw

# Backup current binary
sudo cp /opt/zeroclaw/zeroclaw /opt/zeroclaw/zeroclaw.bak

# Install new version
# (repeat installation step)

# Verify
/opt/zeroclaw/zeroclaw --version

# Start service
sudo systemctl start zeroclaw
```

### Log Rotation

Logs are automatically rotated by the configuration. Manual rotation:

```bash
sudo logrotate -f /etc/logrotate.d/zeroclaw
```

### Backup

```bash
# Backup workspace and config
sudo tar -czf zeroclaw-backup-$(date +%Y%m%d).tar.gz \
    /etc/zeroclaw \
    /var/lib/zeroclaw/workspace \
    /var/lib/zeroclaw/keys

# Store backup securely (off-system recommended)
```

### Key Rotation

```bash
# Rotate encryption key (requires re-encryption of data)
sudo systemctl stop zeroclaw
sudo openssl rand -base64 32 | sudo tee /var/lib/zeroclaw/keys/master.key.new > /dev/null
# Migrate data if needed, then:
sudo mv /var/lib/zeroclaw/keys/master.key.new /var/lib/zeroclaw/keys/master.key
sudo chmod 600 /var/lib/zeroclaw/keys/master.key
sudo systemctl start zeroclaw
```

---

## Performance Tuning

### For Older Hardware (< 1GB RAM)

Edit `/etc/zeroclaw/config.toml`:

```toml
[resources]
max_memory_mb = 256         # Reduce from 512
max_context_tokens = 4096   # Reduce from 8192
max_cpu_percent = 30        # Reduce from 50

[commands]
timeout_seconds = 60        # Increase timeout
max_concurrent = 1          # Reduce concurrency
```

### For Better Performance

```toml
[resources]
max_memory_mb = 1024
max_context_tokens = 16384
max_cpu_percent = 80

[commands]
max_concurrent = 4
```

### Swap Configuration

For systems with limited RAM:

```bash
# Create swap file
sudo fallocate -l 1G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile

# Make permanent
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

---

## Security Checklist Quick Reference

- [ ] Service running as `zeroclaw` user (not root)
- [ ] Bound to `127.0.0.1` only
- [ ] Workspace-only mode enabled
- [ ] Command allowlist enabled
- [ ] SkillForge disabled
- [ ] Audit logging enabled
- [ ] Key files have 600 permissions
- [ ] OS-level sandboxing installed
- [ ] Systemd security score < 3.0

See `SECURITY_CHECKLIST.md` for the complete checklist.

---

## Support

For issues with this hardened configuration package:
1. Check this guide's troubleshooting section
2. Run `security_verify.sh` to identify issues
3. Review logs: `sudo journalctl -u zeroclaw`

For ZeroClaw software issues:
- Documentation: https://github.com/zeroclaw/zeroclaw
- Issues: https://github.com/zeroclaw/zeroclaw/issues
