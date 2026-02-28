# ZeroClaw Security Checklist

Use this checklist before and after deploying ZeroClaw to ensure maximum security.

---

## Pre-Deployment Checklist

### System Preparation

- [ ] Ubuntu/Debian system is up to date (`sudo apt update && sudo apt upgrade`)
- [ ] Unnecessary services are disabled
- [ ] Firewall is configured (`ufw enable`)
- [ ] SSH hardened (key-only auth, non-standard port if exposed)
- [ ] System has adequate disk space (>1GB free)
- [ ] System has adequate RAM (>512MB)

### User and Permissions

- [ ] Dedicated `zeroclaw` system user created
- [ ] User has no login shell (`/usr/sbin/nologin`)
- [ ] User cannot sudo
- [ ] User home directory is `/var/lib/zeroclaw`

### Directory Structure

- [ ] `/opt/zeroclaw` - Binary location (755)
- [ ] `/etc/zeroclaw` - Config location (755)
- [ ] `/var/lib/zeroclaw` - Data directory (750)
- [ ] `/var/lib/zeroclaw/keys` - Keys directory (700)
- [ ] `/var/lib/zeroclaw/workspace` - Workspace (750)
- [ ] `/var/log/zeroclaw` - Logs directory (750)

### Configuration Review

- [ ] `bind_address` is `127.0.0.1` (NOT `0.0.0.0`)
- [ ] `workspace_only` is `true`
- [ ] `use_allowlist` is `true`
- [ ] `skillforge.enabled` is `false`
- [ ] `autonomy.level` is `supervised` or `readonly`
- [ ] `allow_outbound_requests` is `false`
- [ ] `logging.audit.enabled` is `true`
- [ ] `rate_limiting.enabled` is `true`
- [ ] Forbidden paths include `/etc`, `/root`, `/home`, etc.
- [ ] Blocked commands include `sudo`, `curl`, `wget`, etc.
- [ ] No sensitive data in allowed file extensions

### Key Security

- [ ] Master key generated with cryptographically secure random
- [ ] Master key file permissions are 600
- [ ] Master key owned by `zeroclaw` user
- [ ] API key generated
- [ ] API key file permissions are 600
- [ ] Keys are backed up securely (off-system)
- [ ] Backup is encrypted and access-controlled

---

## Post-Deployment Checklist

### Verification Tests

- [ ] Run `security_verify.sh` with all checks passing
- [ ] Service starts successfully
- [ ] Service runs as `zeroclaw` user (not root)
- [ ] Service bound to localhost only (verify with `ss -tlnp`)
- [ ] Cannot access from other machines on network
- [ ] Audit logs being written
- [ ] Command execution respects allowlist
- [ ] File access respects workspace-only mode
- [ ] Forbidden paths are truly inaccessible

### Systemd Hardening

- [ ] `NoNewPrivileges=true` in service file
- [ ] `ProtectSystem=strict` in service file
- [ ] `ProtectHome=true` in service file
- [ ] `PrivateTmp=true` in service file
- [ ] `PrivateDevices=true` in service file
- [ ] Capabilities dropped
- [ ] Seccomp filtering enabled
- [ ] `systemd-analyze security zeroclaw` score < 3.0

### OS-Level Sandboxing

- [ ] Firejail OR Bubblewrap installed
- [ ] Sandbox profile/wrapper configured
- [ ] Service running within sandbox
- [ ] Sandbox restricts filesystem access
- [ ] Sandbox restricts network access
- [ ] Sandbox drops capabilities

---

## Ongoing Security Checklist

### Daily/Weekly

- [ ] Review audit logs for suspicious activity
- [ ] Check service status (`systemctl status zeroclaw`)
- [ ] Verify no unexpected files in workspace
- [ ] Monitor system resource usage

### Monthly

- [ ] Apply OS security updates
- [ ] Check for ZeroClaw updates
- [ ] Review and rotate API keys
- [ ] Review allowed commands list
- [ ] Re-run `security_verify.sh`
- [ ] Review audit log archives

### Quarterly

- [ ] Rotate master encryption key
- [ ] Full security audit
- [ ] Review and update documentation
- [ ] Test backup restoration
- [ ] Update this checklist based on new threats

---

## Incident Response Checklist

### If Suspicious Activity Detected

1. [ ] Stop the service immediately: `sudo systemctl stop zeroclaw`
2. [ ] Preserve logs: `sudo cp -r /var/log/zeroclaw /root/zeroclaw-incident-$(date +%Y%m%d)`
3. [ ] Check for unauthorized files in workspace
4. [ ] Review audit logs for timeline
5. [ ] Check system for compromise indicators
6. [ ] If compromised:
   - [ ] Isolate system from network
   - [ ] Rotate all keys and credentials
   - [ ] Consider reinstalling from scratch
7. [ ] Document incident
8. [ ] Review and strengthen configuration

### Emergency Kill Switch

```bash
# Create kill switch file to immediately stop ZeroClaw
sudo touch /var/lib/zeroclaw/KILL_SWITCH

# Or force stop service
sudo systemctl kill -s SIGKILL zeroclaw
```

---

## Security Anti-Patterns (DON'T DO THESE)

- ❌ Run ZeroClaw as root
- ❌ Bind to 0.0.0.0 without TLS and authentication
- ❌ Disable workspace-only mode
- ❌ Disable command allowlisting
- ❌ Enable SkillForge in production
- ❌ Store API keys or passwords in workspace
- ❌ Use predictable or weak encryption keys
- ❌ Disable audit logging
- ❌ Set world-readable permissions on keys
- ❌ Allow outbound network without strict filtering
- ❌ Use `autonomous` mode in production
- ❌ Skip OS-level sandboxing

---

## Quick Security Commands

```bash
# Check service status
sudo systemctl status zeroclaw

# Check what user service is running as
ps aux | grep zeroclaw

# Check network binding
ss -tlnp | grep zeroclaw

# Check key permissions
ls -la /var/lib/zeroclaw/keys/

# Check systemd security score
sudo systemd-analyze security zeroclaw

# Run full security verification
sudo ./security_verify.sh

# View recent audit logs
sudo tail -100 /var/log/zeroclaw/audit.log

# Emergency stop
sudo systemctl stop zeroclaw
```

---

## Compliance Notes

This hardened configuration helps achieve:

- **Principle of Least Privilege**: Limited user, restricted commands, workspace-only access
- **Defense in Depth**: Multiple layers (config, systemd, OS sandbox)
- **Audit Trail**: Comprehensive logging of all operations
- **Data Protection**: Encryption keys with strict access control
- **Input Validation**: Rate limiting, input size limits, pattern blocking

---

*Last Updated: 2026-02-19*
