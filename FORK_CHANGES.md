# Fork Changes from Upstream ZeroClaw

This document tracks all changes made in this security-hardened fork compared to the upstream [zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw) repository.

## Summary

This fork adds a comprehensive security hardening layer on top of ZeroClaw without modifying the core Rust codebase. All changes are additive and can be easily merged with upstream updates.

## Added Files

### Root Directory

| File | Purpose |
|------|--------|
| `FORK_README.md` | Fork-specific README explaining security features |
| `FORK_CHANGES.md` | This file - documents all fork changes |
| `HARDENING_GUIDE.md` | Comprehensive hardening documentation |
| `DEPLOYMENT_CHECKLIST.md` | Pre/post deployment checklist |
| `deploy.sh` | One-command secure deployment script |

### `security/` Directory (New)

```
security/
├── hardened_config.toml      # Production-ready secure configuration
├── allowed_commands.txt      # Pre-vetted command allowlist (150+ commands)
├── security_verify.sh        # Automated security verification script
├── SECURITY_CHECKLIST.md     # Security verification checklist
├── DEPLOYMENT_GUIDE.md       # Detailed deployment documentation
├── systemd/
│   └── zeroclaw-hardened.service  # Hardened systemd service file
└── sandboxing/
    ├── firejail_profile.profile   # Firejail sandboxing profile
    └── bubblewrap_wrapper.sh      # Bubblewrap wrapper script
```

### `.github/workflows/` (Added)

| File | Purpose |
|------|--------|
| `security-audit.yml` | Automated security scanning workflow |

### `tests/` Directory (Added)

| File | Purpose |
|------|--------|
| `test_security.sh` | Security regression tests |
| `smoke_tests.sh` | Basic functionality smoke tests |

## Modified Files

### `SECURITY.md`
- **Added**: Section on hardened deployment options
- **Added**: References to security/ directory
- **Added**: Comparison table with vanilla ZeroClaw

### `.gitignore`
- **Added**: Security-related ignores (API keys, secrets)

## Configuration Differences

### Default Configuration Changes

| Setting | Upstream Default | Fork Default | Reason |
|---------|-----------------|--------------|--------|
| `autonomy.level` | `autonomous` | `supervised` | Require human approval for destructive ops |
| `filesystem.workspace_only` | `false` | `true` | Prevent filesystem escape |
| `server.bind_address` | `0.0.0.0` | `127.0.0.1` | Localhost-only by default |
| `security.strict_mode` | `false` | `true` | Enable all security validations |
| `security.allow_remote_skills` | `true` | `false` | Prevent loading untrusted skills |

## Systemd Security Directives

The hardened systemd service adds the following directives not present in standard deployments:

```ini
# Privilege isolation
NoNewPrivileges=true
CapabilityBoundingSet=
AmbientCapabilities=

# Filesystem isolation  
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true

# Kernel protection
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectKernelLogs=true
ProtectControlGroups=true

# Syscall filtering
SystemCallFilter=@system-service
SystemCallFilter=~@mount @reboot @swap @module @raw-io @clock @cpu-emulation @debug @obsolete @privileged

# Resource limits
MemoryMax=512M
CPUQuota=50%
TasksMax=20
```

## Upstream Compatibility

### Merging Upstream Changes

This fork is designed to be easily synced with upstream:

```bash
git fetch upstream
git merge upstream/main
# Conflicts only in security-related files (intentional)
git checkout --ours security/ FORK_*.md HARDENING_GUIDE.md DEPLOYMENT_CHECKLIST.md deploy.sh
```

### Breaking Changes

None. All fork changes are additive or configuration-only.

## Version Tracking

| Fork Version | Upstream Version | Date | Notes |
|--------------|-----------------|------|-------|
| v1.0.0-secure | v0.1.x | 2026-02-19 | Initial hardened release |

## Future Plans

- [ ] AppArmor profile for additional MAC enforcement
- [ ] SELinux policy module
- [ ] Automated CVE monitoring
- [ ] Integration with security scanning tools
- [ ] Kubernetes security contexts
