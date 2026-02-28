# ZeroClaw Secure Fork

[![Security Audit](https://i.ytimg.com/vi/jfL6I0VDgGw/hq720.jpg?sqp=-oaymwEhCK4FEIIDSFryq4qpAxMIARUAAAAAGAElAADIQj0AgKJD&rs=AOn4CLCDIgyqNGN9bFR2zNmXseZOxGqRGw)

> A production-ready, security-hardened fork of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) for maximum security deployments.

## Why This Fork?

While vanilla ZeroClaw is already well-designed for security, this fork provides:

1. **Pre-configured hardened settings** - Paranoid-mode defaults that prioritize security
2. **Defense in depth** - Multiple security layers from application to OS level
3. **One-command deployment** - Automated secure installation script
4. **Comprehensive documentation** - Security guides, checklists, and best practices
5. **Validation tools** - Automated security verification scripts

## Security Comparison

| Feature | OpenClaw | Vanilla ZeroClaw | This Fork |
|---------|----------|------------------|------------|
| Language | Python | Rust | Rust |
| Memory Safety | No | Yes | Yes |
| Default Autonomy | Full | Configurable | Supervised |
| Workspace Isolation | Limited | Yes | Enforced |
| Command Allowlisting | No | Yes | Pre-configured |
| Systemd Hardening | No | Basic | 15+ directives |
| OS Sandboxing | No | No | Included |
| Security Verification | No | No | Automated |
| Memory Footprint | >1GB | <5MB | <5MB |

## Quick Start

### One-Command Install

```bash
git clone https://github.com/YOUR_USERNAME/zeroclaw-secure.git
cd zeroclaw-secure
sudo ./deploy.sh
```

### Manual Install

```bash
# Build from source
cargo build --release

# Apply hardened config
sudo mkdir -p /etc/zeroclaw
sudo cp security/hardened_config.toml /etc/zeroclaw/config.toml

# Install systemd service
sudo cp security/systemd/zeroclaw-hardened.service /etc/systemd/system/zeroclaw.service
sudo systemctl daemon-reload
sudo systemctl enable --now zeroclaw

# Verify security
./security/security_verify.sh
```

## What's Included

```
security/
├── hardened_config.toml      # Paranoid-mode configuration
├── allowed_commands.txt      # Pre-vetted safe commands
├── security_verify.sh        # Automated security checks
├── SECURITY_CHECKLIST.md     # Pre-deployment checklist
├── DEPLOYMENT_GUIDE.md       # Detailed deployment guide
├── systemd/
│   └── zeroclaw-hardened.service  # Systemd with 15+ security directives
└── sandboxing/
    ├── firejail_profile.profile   # Firejail sandboxing
    └── bubblewrap_wrapper.sh      # Bubblewrap isolation
```

## Key Security Features

### Application Layer
- ✅ Supervised autonomy (requires approval for destructive operations)
- ✅ Workspace-only filesystem access
- ✅ Strict command allowlisting (150+ pre-vetted commands)
- ✅ Rate limiting and input validation
- ✅ Localhost-only binding

### Service Layer (Systemd)
- ✅ Runs as unprivileged `zeroclaw` user
- ✅ `NoNewPrivileges=true`
- ✅ `ProtectSystem=strict`
- ✅ `PrivateTmp=true`
- ✅ `CapabilityBoundingSet=` (no capabilities)
- ✅ Seccomp syscall filtering
- ✅ Memory and CPU limits

### OS Layer (Optional)
- ✅ Firejail profile with seccomp
- ✅ Bubblewrap wrapper for container isolation
- ✅ AppArmor-compatible

## Documentation

- [HARDENING_GUIDE.md](HARDENING_GUIDE.md) - Detailed hardening instructions
- [DEPLOYMENT_CHECKLIST.md](DEPLOYMENT_CHECKLIST.md) - Pre/post deployment checklist
- [FORK_CHANGES.md](FORK_CHANGES.md) - All changes from upstream
- [security/SECURITY_CHECKLIST.md](security/SECURITY_CHECKLIST.md) - Security verification checklist

## Syncing with Upstream

To get updates from the original ZeroClaw:

```bash
# Add upstream remote
git remote add upstream https://github.com/zeroclaw-labs/zeroclaw.git

# Fetch and merge
git fetch upstream
git merge upstream/main

# Resolve conflicts (security/ directory should be kept)
git checkout --ours security/
git add security/
git commit -m "Merge upstream, preserving security configurations"
```

## Contributing

We welcome security improvements!

1. Fork this repository
2. Create a feature branch (`git checkout -b security/improvement`)
3. Make your changes
4. Run security verification (`./security/security_verify.sh`)
5. Submit a pull request

### Security Reports

For security vulnerabilities, please use [GitHub Security Advisories](../../security/advisories/new) instead of public issues.

## License

Same as upstream ZeroClaw - see [LICENSE](LICENSE).

## Acknowledgments

- [zeroclaw-labs](https://github.com/zeroclaw-labs) for the excellent original project
- The Rust community for secure-by-default tooling
- All contributors to this security fork
