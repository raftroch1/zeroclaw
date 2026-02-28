# ZeroClaw Hardening Guide

This guide provides comprehensive instructions for deploying ZeroClaw with maximum security hardening on Ubuntu systems.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Security Architecture](#security-architecture)
- [Configuration Guide](#configuration-guide)
- [Deployment Options](#deployment-options)
- [Sandboxing Options](#sandboxing-options)
- [Monitoring & Maintenance](#monitoring--maintenance)
- [Troubleshooting](#troubleshooting)

## Overview

### Security Philosophy

This hardened fork follows the principle of **defense in depth**:

1. **Application Layer** - Hardened configuration with restrictive defaults
2. **Service Layer** - Systemd sandboxing with 15+ security directives
3. **OS Layer** - Optional Firejail/Bubblewrap containerization
4. **Network Layer** - Localhost-only binding, no external exposure

### Security Improvements Over Vanilla ZeroClaw

| Feature | Vanilla | Hardened Fork |
|---------|---------|---------------|
| Default autonomy | Full | Supervised |
| Workspace isolation | Optional | Mandatory |
| Command allowlisting | User-configured | Pre-configured safe list |
| Systemd hardening | Basic | 15+ security directives |
| OS sandboxing | Not included | Firejail/Bubblewrap profiles |
| Security verification | Manual | Automated script |

## Quick Start

```bash
# Clone this fork
git clone https://github.com/YOUR_USERNAME/zeroclaw-secure.git
cd zeroclaw-secure

# Run the deployment script
sudo ./deploy.sh

# Verify security
./security/security_verify.sh
```

## Security Architecture

### Layer 1: Application Configuration

The hardened configuration (`security/hardened_config.toml`) includes:

- **Supervised autonomy mode** - Destructive operations require approval
- **Workspace-only filesystem access** - Cannot escape designated directory
- **Strict command allowlisting** - Only pre-approved commands can execute
- **Rate limiting** - Prevents abuse and runaway operations
- **Input validation** - Blocks injection patterns

### Layer 2: Systemd Sandboxing

The systemd service (`security/systemd/zeroclaw-hardened.service`) provides:

```ini
# Key security directives
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
CapabilityBoundingSet=
SystemCallFilter=@system-service
```

Run `systemd-analyze security zeroclaw` to verify (target score: < 2.0).

### Layer 3: OS Sandboxing (Optional)

For additional isolation:

- **Firejail** - Application sandboxing with seccomp
- **Bubblewrap** - Lightweight container isolation

See `security/sandboxing/` for profiles.

## Configuration Guide

### Essential Settings

```toml
# CRITICAL - Never change these
[server]
bind_address = "127.0.0.1"  # Localhost only

[autonomy]
level = "supervised"        # Require approval for destructive ops

[filesystem]
workspace_only = true       # Prevent escape
```

### Command Allowlisting

Edit `security/allowed_commands.txt` to customize allowed commands:

```
# Safe commands (one per line)
ls
cat
head
tail
grep
# Add more as needed
```

### API Key Security

Your API keys should be stored securely:

```bash
# Create secure key storage
sudo mkdir -p /etc/zeroclaw/secrets
sudo chmod 700 /etc/zeroclaw/secrets

# Store API key
echo "your-api-key" | sudo tee /etc/zeroclaw/secrets/api_key > /dev/null
sudo chmod 600 /etc/zeroclaw/secrets/api_key
```

## Deployment Options

### Option 1: Systemd Service (Recommended)

```bash
# Install service
sudo cp security/systemd/zeroclaw-hardened.service /etc/systemd/system/zeroclaw.service
sudo systemctl daemon-reload
sudo systemctl enable --now zeroclaw

# Verify
sudo systemctl status zeroclaw
systemd-analyze security zeroclaw
```

### Option 2: Docker (Alternative)

```bash
# Build with hardened config
docker build -t zeroclaw-secure .

# Run read-only with minimal privileges
docker run --read-only \
  --security-opt=no-new-privileges \
  --cap-drop=ALL \
  -v /path/to/workspace:/workspace \
  zeroclaw-secure
```

### Option 3: Firejail Sandboxing

```bash
# Install firejail
sudo apt install firejail

# Run with profile
firejail --profile=security/sandboxing/firejail_profile.profile zeroclaw
```

## Sandboxing Options

### Firejail Profile

The included Firejail profile (`security/sandboxing/firejail_profile.profile`):

- Blocks home directory access
- Restricts network to localhost
- Enables seccomp filtering
- Uses private /tmp and /dev
- Drops all capabilities

### Bubblewrap Wrapper

For lighter containerization (`security/sandboxing/bubblewrap_wrapper.sh`):

```bash
# Run with bubblewrap
./security/sandboxing/bubblewrap_wrapper.sh zeroclaw agent
```

## Monitoring & Maintenance

### Security Verification

Run regularly:

```bash
# Full security audit
./security/security_verify.sh

# Systemd security score
systemd-analyze security zeroclaw

# Check for running anomalies
journalctl -u zeroclaw --since "1 hour ago" | grep -i error
```

### Log Monitoring

```bash
# Real-time logs
journalctl -u zeroclaw -f

# Security-relevant events
journalctl -u zeroclaw | grep -E "(denied|blocked|violation|error)"
```

### Updating

```bash
# Update from upstream (preserving security config)
git fetch upstream
git merge upstream/main --no-commit
# Review changes before committing
git diff --staged
git commit -m "Merge upstream changes"

# Rebuild
cargo build --release
sudo systemctl restart zeroclaw
```

## Troubleshooting

### Common Issues

#### "Permission denied" errors

The hardened configuration restricts filesystem access:

```bash
# Check workspace permissions
ls -la /var/lib/zeroclaw/workspace

# Ensure zeroclaw user owns workspace
sudo chown -R zeroclaw:zeroclaw /var/lib/zeroclaw
```

#### "Command not allowed" errors

Add the command to the allowlist:

```bash
# Edit allowed commands
sudo nano /etc/zeroclaw/allowed_commands.txt
# Add the command, restart service
sudo systemctl restart zeroclaw
```

#### Systemd service won't start

```bash
# Check detailed status
sudo systemctl status zeroclaw -l

# Check journal
journalctl -u zeroclaw -n 50

# Verify user exists
id zeroclaw || sudo useradd -r -s /bin/false zeroclaw
```

### Security Score Too High

If `systemd-analyze security` shows a high score:

1. Ensure all security directives are present in service file
2. Check for conflicting directives
3. Some directives may not work on older kernels

## Further Reading

- [ZeroClaw Documentation](https://github.com/zeroclaw-labs/zeroclaw)
- [systemd Security Hardening](https://www.freedesktop.org/software/systemd/man/systemd.exec.html)
- [Firejail Documentation](https://firejail.wordpress.com/documentation-2/)
- [OWASP Secure Coding Practices](https://owasp.org/www-project-secure-coding-practices-quick-reference-guide/)
