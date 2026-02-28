# Security Policy

## Reporting Security Vulnerabilities

**Please do NOT report security vulnerabilities through public GitHub issues.**

### Reporting Process

1. **Preferred Method**: Use [GitHub Security Advisories](../../security/advisories/new)
2. **Alternative**: Email the maintainers directly (see GitHub profiles)

### Information to Include

- Type of vulnerability (e.g., RCE, path traversal, injection)
- Location of the vulnerable code (file path and line numbers)
- Step-by-step reproduction instructions
- Proof-of-concept or exploit code (if available)
- Impact assessment
- Suggested remediation (if any)

### Response Timeline

| Stage | Timeline |
|-------|----------|
| Acknowledgment | 48 hours |
| Initial assessment | 1 week |
| Remediation (critical) | 2 weeks |
| Remediation (high) | 4 weeks |
| Remediation (medium/low) | 8 weeks |

### Scope

This security policy covers:

- The ZeroClaw application code
- Configuration files in this repository
- Docker images built from this repository
- Documentation that could lead to insecure configurations

### Out of Scope

- Third-party dependencies (report to upstream maintainers)
- Social engineering attacks
- Physical security
- Issues in test/example code that is not meant for production

## Security Features

This hardened fork includes:

### Defense Layers

1. **Application Security**
   - Supervised autonomy mode (destructive ops require approval)
   - Workspace-only filesystem access
   - Strict command allowlisting
   - Input validation and sanitization
   - Rate limiting

2. **Service Security (Systemd)**
   - Unprivileged user execution
   - Capability dropping
   - Filesystem isolation
   - Network restrictions
   - Seccomp syscall filtering
   - Resource limits

3. **Optional OS Sandboxing**
   - Firejail profiles
   - Bubblewrap containers
   - AppArmor compatibility

### Security Testing

- Automated security audit workflow (`.github/workflows/security-audit.yml`)
- Dependency vulnerability scanning
- Security regression tests
- Manual penetration testing on releases

## Supported Versions

| Version | Supported | Notes |
|---------|-----------|-------|
| 1.0.x-secure | ✅ | Current stable release |
| < 1.0.0 | ❌ | Pre-release versions |

## Security Best Practices for Users

### Do

- Use the hardened configuration file
- Run as a non-root user via systemd
- Enable all sandboxing options compatible with your system
- Keep ZeroClaw and dependencies updated
- Monitor logs for security events
- Use strong, unique API keys
- Limit workspace to necessary directories

### Don't

- Run as root or with sudo
- Disable workspace isolation
- Allow all commands (use allowlist)
- Expose the service to the network without authentication
- Store API keys in configuration files
- Ignore security warnings in logs

## Acknowledgments

We appreciate responsible disclosure and will acknowledge security researchers who report valid vulnerabilities (with their permission) in our security advisories.
