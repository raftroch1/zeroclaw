# Changelog

All notable changes to the ZeroClaw Secure Fork will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0-secure] - 2026-02-19

### Added

#### Security Hardening
- Comprehensive hardened configuration file (`security/hardened_config.toml`)
  - Supervised autonomy mode as default
  - Workspace-only filesystem access enforced
  - Localhost-only network binding
  - Strict input validation
  - Rate limiting enabled
  - Remote skills disabled

#### Systemd Hardening
- Production-ready systemd service file with 15+ security directives
  - `NoNewPrivileges=true`
  - `ProtectSystem=strict`
  - `PrivateTmp=true`
  - `PrivateDevices=true`
  - Full capability dropping
  - Seccomp syscall filtering
  - Memory and CPU resource limits

#### OS-Level Sandboxing
- Firejail profile for application sandboxing
- Bubblewrap wrapper script for container isolation

#### Automation
- One-command deployment script (`deploy.sh`)
  - Automatic Ubuntu version detection
  - Idempotent installation (safe to run multiple times)
  - Rust toolchain installation
  - Systemd service setup
  - Security verification

#### Documentation
- `HARDENING_GUIDE.md` - Comprehensive security hardening guide
- `DEPLOYMENT_CHECKLIST.md` - Pre/post deployment checklist
- `FORK_README.md` - Fork-specific documentation
- `FORK_CHANGES.md` - Upstream diff documentation
- `SECURITY_POLICY.md` - Security reporting policy
- `security/SECURITY_CHECKLIST.md` - Security verification checklist
- `security/DEPLOYMENT_GUIDE.md` - Detailed deployment guide

#### Testing
- Security regression test suite (`tests/test_security.sh`)
- Smoke tests for basic functionality (`tests/smoke_tests.sh`)
- Security verification script (`security/security_verify.sh`)

#### CI/CD
- GitHub Actions security audit workflow
  - Cargo audit for dependency vulnerabilities
  - Cargo deny for license compliance
  - Clippy static analysis
  - Semgrep SAST scanning
  - Docker image security scanning
  - SBOM generation

### Changed
- Default configuration now prioritizes security over convenience
- Updated `.gitignore` to exclude security-sensitive files

### Security
- All default settings now follow principle of least privilege
- Defense-in-depth architecture with multiple security layers
- Pre-vetted command allowlist (150+ safe commands)

## [Upstream] - Tracking zeroclaw-labs/zeroclaw

This fork tracks the upstream ZeroClaw repository. See [FORK_CHANGES.md](FORK_CHANGES.md) for differences.
