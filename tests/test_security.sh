#!/bin/bash
# =============================================================================
# ZeroClaw Security Test Suite
# =============================================================================
# Runs security regression tests to ensure hardening is effective.
# =============================================================================

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASSED=0
FAILED=0
WARNINGS=0

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    PASSED=$((PASSED + 1))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    FAILED=$((FAILED + 1))
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
    WARNINGS=$((WARNINGS + 1))
}

log_test() {
    echo -e "\n[TEST] $1"
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "====================================="
echo "ZeroClaw Security Test Suite"
echo "====================================="
echo ""

# -----------------------------------------------------------------------------
# Configuration File Tests
# -----------------------------------------------------------------------------
log_test "Checking hardened configuration exists..."
if [ -f "$PROJECT_DIR/security/hardened_config.toml" ]; then
    log_pass "Hardened config file exists"
else
    log_fail "Hardened config file missing"
fi

log_test "Checking config has supervised autonomy..."
if grep -q 'level = "supervised"' "$PROJECT_DIR/security/hardened_config.toml" 2>/dev/null; then
    log_pass "Autonomy level is supervised"
else
    log_fail "Autonomy level not set to supervised"
fi

log_test "Checking config has workspace_only enabled..."
if grep -q 'workspace_only = true' "$PROJECT_DIR/security/hardened_config.toml" 2>/dev/null; then
    log_pass "Workspace isolation enabled"
else
    log_fail "Workspace isolation not enabled"
fi

log_test "Checking config binds to localhost only..."
if grep -q 'bind_address = "127.0.0.1"' "$PROJECT_DIR/security/hardened_config.toml" 2>/dev/null; then
    log_pass "Bound to localhost only"
else
    log_fail "Not bound to localhost - potential network exposure"
fi

log_test "Checking remote skills are disabled..."
if grep -q 'allow_remote_skills = false' "$PROJECT_DIR/security/hardened_config.toml" 2>/dev/null; then
    log_pass "Remote skills disabled"
else
    log_warn "Remote skills setting not found or enabled"
fi

# -----------------------------------------------------------------------------
# Systemd Service Tests
# -----------------------------------------------------------------------------
log_test "Checking systemd service file exists..."
if [ -f "$PROJECT_DIR/security/systemd/zeroclaw-hardened.service" ]; then
    log_pass "Systemd service file exists"
else
    log_fail "Systemd service file missing"
fi

log_test "Checking for NoNewPrivileges directive..."
if grep -q 'NoNewPrivileges=true' "$PROJECT_DIR/security/systemd/zeroclaw-hardened.service" 2>/dev/null; then
    log_pass "NoNewPrivileges enabled"
else
    log_fail "NoNewPrivileges not enabled"
fi

log_test "Checking for ProtectSystem directive..."
if grep -q 'ProtectSystem=strict' "$PROJECT_DIR/security/systemd/zeroclaw-hardened.service" 2>/dev/null; then
    log_pass "ProtectSystem=strict enabled"
else
    log_fail "ProtectSystem not set to strict"
fi

log_test "Checking for PrivateTmp directive..."
if grep -q 'PrivateTmp=true' "$PROJECT_DIR/security/systemd/zeroclaw-hardened.service" 2>/dev/null; then
    log_pass "PrivateTmp enabled"
else
    log_fail "PrivateTmp not enabled"
fi

log_test "Checking for capability dropping..."
if grep -q 'CapabilityBoundingSet=' "$PROJECT_DIR/security/systemd/zeroclaw-hardened.service" 2>/dev/null; then
    log_pass "Capabilities dropped"
else
    log_fail "Capabilities not dropped"
fi

log_test "Checking for syscall filtering..."
if grep -q 'SystemCallFilter=' "$PROJECT_DIR/security/systemd/zeroclaw-hardened.service" 2>/dev/null; then
    log_pass "Syscall filtering enabled"
else
    log_fail "Syscall filtering not configured"
fi

log_test "Checking service runs as non-root..."
if grep -q 'User=zeroclaw' "$PROJECT_DIR/security/systemd/zeroclaw-hardened.service" 2>/dev/null; then
    log_pass "Service runs as non-root user"
else
    log_fail "Service may run as root"
fi

# -----------------------------------------------------------------------------
# Sandboxing Profile Tests
# -----------------------------------------------------------------------------
log_test "Checking Firejail profile exists..."
if [ -f "$PROJECT_DIR/security/sandboxing/firejail_profile.profile" ]; then
    log_pass "Firejail profile exists"
else
    log_warn "Firejail profile missing (optional)"
fi

log_test "Checking Bubblewrap wrapper exists..."
if [ -f "$PROJECT_DIR/security/sandboxing/bubblewrap_wrapper.sh" ]; then
    log_pass "Bubblewrap wrapper exists"
else
    log_warn "Bubblewrap wrapper missing (optional)"
fi

# -----------------------------------------------------------------------------
# Allowed Commands Tests
# -----------------------------------------------------------------------------
log_test "Checking allowed commands list exists..."
if [ -f "$PROJECT_DIR/security/allowed_commands.txt" ]; then
    log_pass "Allowed commands list exists"
else
    log_fail "Allowed commands list missing"
fi

log_test "Checking dangerous commands are NOT in allowlist..."
DANGEROUS_CMDS=("rm -rf" "dd if" "mkfs" "fdisk" "shutdown" "reboot" "init")
DANGEROUS_FOUND=false
for cmd in "${DANGEROUS_CMDS[@]}"; do
    if grep -q "^$cmd$" "$PROJECT_DIR/security/allowed_commands.txt" 2>/dev/null; then
        log_fail "Dangerous command in allowlist: $cmd"
        DANGEROUS_FOUND=true
    fi
done
if [ "$DANGEROUS_FOUND" = false ]; then
    log_pass "No dangerous commands in allowlist"
fi

# -----------------------------------------------------------------------------
# Documentation Tests
# -----------------------------------------------------------------------------
log_test "Checking security documentation exists..."
DOCS=("HARDENING_GUIDE.md" "DEPLOYMENT_CHECKLIST.md" "FORK_README.md" "SECURITY_POLICY.md")
for doc in "${DOCS[@]}"; do
    if [ -f "$PROJECT_DIR/$doc" ]; then
        log_pass "Documentation exists: $doc"
    else
        log_warn "Documentation missing: $doc"
    fi
done

# -----------------------------------------------------------------------------
# Script Tests
# -----------------------------------------------------------------------------
log_test "Checking deploy script exists and is executable..."
if [ -x "$PROJECT_DIR/deploy.sh" ]; then
    log_pass "deploy.sh exists and is executable"
else
    log_fail "deploy.sh missing or not executable"
fi

log_test "Checking security_verify.sh exists and is executable..."
if [ -x "$PROJECT_DIR/security/security_verify.sh" ]; then
    log_pass "security_verify.sh exists and is executable"
else
    log_fail "security_verify.sh missing or not executable"
fi

# -----------------------------------------------------------------------------
# Summary
# -----------------------------------------------------------------------------
echo ""
echo "====================================="
echo "Test Results Summary"
echo "====================================="
echo -e "${GREEN}Passed:${NC} $PASSED"
echo -e "${RED}Failed:${NC} $FAILED"
echo -e "${YELLOW}Warnings:${NC} $WARNINGS"
echo ""

if [ $FAILED -gt 0 ]; then
    echo -e "${RED}Some security tests failed!${NC}"
    exit 1
else
    echo -e "${GREEN}All security tests passed!${NC}"
    exit 0
fi
