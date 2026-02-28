# ZeroClaw Secure Deployment Checklist

Use this checklist for every deployment to ensure maximum security.

## Pre-Deployment

### System Preparation

- [ ] Ubuntu 20.04+ or Debian 11+ installed
- [ ] System fully updated (`sudo apt update && sudo apt upgrade`)
- [ ] Firewall enabled (`sudo ufw enable`)
- [ ] SSH hardened (key-only auth, no root login)
- [ ] Automatic security updates enabled

### Dependencies

- [ ] Rust toolchain installed (1.70+)
- [ ] Build essentials: `sudo apt install build-essential pkg-config libssl-dev`
- [ ] (Optional) Firejail: `sudo apt install firejail`
- [ ] (Optional) Bubblewrap: `sudo apt install bubblewrap`

### Security Review

- [ ] Reviewed `security/hardened_config.toml` settings
- [ ] Customized `security/allowed_commands.txt` for your use case
- [ ] API keys stored securely (not in config files)
- [ ] Network exposure requirements understood

## Deployment Steps

### Option A: Automated (Recommended)

- [ ] Clone repository
- [ ] Run `sudo ./deploy.sh`
- [ ] Answer prompts for configuration
- [ ] Review deployment summary

### Option B: Manual

#### 1. Build Application

- [ ] `cargo build --release`
- [ ] Binary created at `target/release/zeroclaw`

#### 2. Create System User

```bash
sudo useradd -r -s /bin/false -d /var/lib/zeroclaw zeroclaw
```

- [ ] User `zeroclaw` created
- [ ] User is system user (no login shell)

#### 3. Create Directories

```bash
sudo mkdir -p /opt/zeroclaw
sudo mkdir -p /etc/zeroclaw
sudo mkdir -p /var/lib/zeroclaw/{workspace,backups,keys}
sudo mkdir -p /var/log/zeroclaw
```

- [ ] Directories created
- [ ] Correct ownership set

#### 4. Install Files

- [ ] Binary copied to `/opt/zeroclaw/zeroclaw`
- [ ] Config copied to `/etc/zeroclaw/config.toml`
- [ ] Allowed commands copied to `/etc/zeroclaw/allowed_commands.txt`
- [ ] Permissions set correctly (0755 binary, 0640 config)

#### 5. Configure API Keys

- [ ] API key stored in `/etc/zeroclaw/secrets/`
- [ ] Key file has 0600 permissions
- [ ] Only zeroclaw user can read

#### 6. Install Systemd Service

```bash
sudo cp security/systemd/zeroclaw-hardened.service /etc/systemd/system/zeroclaw.service
sudo systemctl daemon-reload
```

- [ ] Service file installed
- [ ] Daemon reloaded

## Post-Deployment Verification

### Service Status

- [ ] Service started: `sudo systemctl start zeroclaw`
- [ ] Service enabled: `sudo systemctl enable zeroclaw`
- [ ] No errors in logs: `journalctl -u zeroclaw -n 50`

### Security Verification

- [ ] Run `./security/security_verify.sh`
- [ ] All checks pass or are acceptable

### Systemd Security Score

```bash
systemd-analyze security zeroclaw
```

- [ ] Score is **< 3.0** (Good)
- [ ] Score is **< 2.0** (Excellent - target)
- [ ] No UNSAFE items in critical categories

### Functional Verification

- [ ] ZeroClaw responds to basic commands
- [ ] Workspace access works correctly
- [ ] Blocked commands are properly rejected
- [ ] Rate limiting is functioning

### Network Verification

- [ ] Service only listening on localhost:
  ```bash
  sudo ss -tlnp | grep zeroclaw
  # Should show 127.0.0.1:8080 only
  ```
- [ ] No external exposure (unless intentionally configured)

## Ongoing Maintenance

### Daily

- [ ] Check service status: `systemctl status zeroclaw`
- [ ] Review logs for anomalies

### Weekly

- [ ] Run security verification script
- [ ] Check for ZeroClaw updates
- [ ] Review system security updates

### Monthly

- [ ] Full security audit
- [ ] Review and rotate API keys if needed
- [ ] Test backup and recovery procedures
- [ ] Review allowed commands list

### On Updates

- [ ] Review changelog for security implications
- [ ] Test in staging environment first
- [ ] Run full verification after update

## Security Monitoring

### Log Monitoring Commands

```bash
# Real-time logs
journalctl -u zeroclaw -f

# Security events
journalctl -u zeroclaw | grep -E "(denied|blocked|violation|error|warn)"

# Resource usage
systemctl status zeroclaw --no-pager
```

### Alerts to Configure

- [ ] Service restart alerts
- [ ] Memory usage alerts (>400MB)
- [ ] Error rate alerts
- [ ] Authentication failure alerts

## Emergency Procedures

### Stop Service Immediately

```bash
sudo systemctl stop zeroclaw
```

### Disable Service

```bash
sudo systemctl disable zeroclaw
sudo systemctl mask zeroclaw  # Prevent manual starts
```

### Revoke API Keys

```bash
sudo rm /etc/zeroclaw/secrets/*
sudo systemctl restart zeroclaw
```

### Forensics Mode

```bash
# Preserve logs before investigation
sudo journalctl -u zeroclaw > /tmp/zeroclaw_forensics.log

# Preserve workspace state
sudo tar -czf /tmp/workspace_backup.tar.gz /var/lib/zeroclaw/workspace
```

## Sign-Off

| Step | Verified By | Date |
|------|------------|------|
| Pre-deployment | | |
| Deployment | | |
| Post-deployment | | |
| Security verification | | |
