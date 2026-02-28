#!/bin/bash
# =============================================================================
# ZeroClaw Security Verification Script
# =============================================================================
# Checks the security posture of a ZeroClaw installation
#
# Usage: sudo ./security_verify.sh
#
# Exit codes:
#   0 = All checks passed
#   1 = Warnings present but installation usable
#   2 = Critical security issues found
# =============================================================================

set -uo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration paths
CONFIG_FILE="/etc/zeroclaw/config.toml"
DATA_DIR="/var/lib/zeroclaw"
KEYS_DIR="/var/lib/zeroclaw/keys"
MASTER_KEY="/var/lib/zeroclaw/keys/master.key"
API_KEY="/var/lib/zeroclaw/keys/api.key"
LOG_DIR="/var/log/zeroclaw"
ZEROCLAW_BIN="/opt/zeroclaw/zeroclaw"
SERVICE_FILE="/etc/systemd/system/zeroclaw.service"

# Score tracking
PASS_COUNT=0
WARN_COUNT=0
FAIL_COUNT=0
TOTAL_CHECKS=0

# -----------------------------------------------------------------------------
# Helper Functions
# -----------------------------------------------------------------------------

check_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((PASS_COUNT++))
    ((TOTAL_CHECKS++))
}

check_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
    ((WARN_COUNT++))
    ((TOTAL_CHECKS++))
}

check_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((FAIL_COUNT++))
    ((TOTAL_CHECKS++))
}

check_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

section_header() {
    echo ""
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

# -----------------------------------------------------------------------------
# Check Functions
# -----------------------------------------------------------------------------

check_root() {
    if [[ $EUID -ne 0 ]]; then
        check_warn "Not running as root. Some checks may be incomplete."
    fi
}

check_file_permissions() {
    section_header "FILE PERMISSIONS"
    
    # Config file
    if [[ -f "$CONFIG_FILE" ]]; then
        local perms=$(stat -c %a "$CONFIG_FILE")
        if [[ "$perms" == "640" || "$perms" == "600" ]]; then
            check_pass "Config file permissions: $perms"
        else
            check_fail "Config file permissions too open: $perms (should be 640 or 600)"
        fi
    else
        check_fail "Config file not found: $CONFIG_FILE"
    fi
    
    # Master key
    if [[ -f "$MASTER_KEY" ]]; then
        local perms=$(stat -c %a "$MASTER_KEY")
        if [[ "$perms" == "600" ]]; then
            check_pass "Master key permissions: $perms"
        else
            check_fail "Master key permissions too open: $perms (MUST be 600)"
        fi
        
        # Check owner
        local owner=$(stat -c %U "$MASTER_KEY")
        if [[ "$owner" == "zeroclaw" ]]; then
            check_pass "Master key owner: $owner"
        else
            check_warn "Master key owner is '$owner' (expected: zeroclaw)"
        fi
    else
        check_warn "Master key not found: $MASTER_KEY"
    fi
    
    # API key
    if [[ -f "$API_KEY" ]]; then
        local perms=$(stat -c %a "$API_KEY")
        if [[ "$perms" == "600" ]]; then
            check_pass "API key permissions: $perms"
        else
            check_fail "API key permissions too open: $perms (MUST be 600)"
        fi
    else
        check_warn "API key not found: $API_KEY"
    fi
    
    # Keys directory
    if [[ -d "$KEYS_DIR" ]]; then
        local perms=$(stat -c %a "$KEYS_DIR")
        if [[ "$perms" == "700" ]]; then
            check_pass "Keys directory permissions: $perms"
        else
            check_fail "Keys directory permissions too open: $perms (should be 700)"
        fi
    fi
    
    # Data directory
    if [[ -d "$DATA_DIR" ]]; then
        local perms=$(stat -c %a "$DATA_DIR")
        if [[ "$perms" =~ ^7[0-5]0$ ]]; then
            check_pass "Data directory permissions: $perms"
        else
            check_warn "Data directory permissions may be too open: $perms"
        fi
    fi
}

check_config_security() {
    section_header "CONFIGURATION SECURITY"
    
    if [[ ! -f "$CONFIG_FILE" ]]; then
        check_fail "Cannot check config: file not found"
        return
    fi
    
    # Check bind address
    if grep -q 'bind_address.*=.*"127.0.0.1"' "$CONFIG_FILE"; then
        check_pass "Bind address: localhost only (127.0.0.1)"
    elif grep -q 'bind_address.*=.*"0.0.0.0"' "$CONFIG_FILE"; then
        check_fail "CRITICAL: Bind address is 0.0.0.0 (exposed to network!)"
    else
        check_warn "Could not verify bind address setting"
    fi
    
    # Check workspace_only
    if grep -q 'workspace_only.*=.*true' "$CONFIG_FILE"; then
        check_pass "Workspace-only mode: enabled"
    elif grep -q 'workspace_only.*=.*false' "$CONFIG_FILE"; then
        check_fail "CRITICAL: Workspace-only mode is disabled!"
    else
        check_warn "Could not verify workspace_only setting"
    fi
    
    # Check command allowlist
    if grep -q 'use_allowlist.*=.*true' "$CONFIG_FILE"; then
        check_pass "Command allowlist: enabled"
    elif grep -q 'use_allowlist.*=.*false' "$CONFIG_FILE"; then
        check_fail "CRITICAL: Command allowlist is disabled!"
    else
        check_warn "Could not verify command allowlist setting"
    fi
    
    # Check SkillForge
    if grep -q 'skillforge' "$CONFIG_FILE"; then
        if grep -A5 'skillforge' "$CONFIG_FILE" | grep -q 'enabled.*=.*false'; then
            check_pass "SkillForge: disabled"
        elif grep -A5 'skillforge' "$CONFIG_FILE" | grep -q 'enabled.*=.*true'; then
            check_fail "SECURITY RISK: SkillForge is enabled"
        fi
    else
        check_info "SkillForge setting not found (likely disabled by default)"
    fi
    
    # Check audit logging
    if grep -A5 'logging.audit' "$CONFIG_FILE" | grep -q 'enabled.*=.*true'; then
        check_pass "Audit logging: enabled"
    else
        check_warn "Audit logging may not be enabled"
    fi
    
    # Check rate limiting
    if grep -A5 'rate_limiting' "$CONFIG_FILE" | grep -q 'enabled.*=.*true'; then
        check_pass "Rate limiting: enabled"
    else
        check_warn "Rate limiting may not be enabled"
    fi
    
    # Check autonomy level
    if grep -q 'level.*=.*"supervised"' "$CONFIG_FILE"; then
        check_pass "Autonomy level: supervised"
    elif grep -q 'level.*=.*"readonly"' "$CONFIG_FILE"; then
        check_pass "Autonomy level: readonly (most restrictive)"
    elif grep -q 'level.*=.*"autonomous"' "$CONFIG_FILE"; then
        check_warn "Autonomy level is 'autonomous' - consider using 'supervised'"
    fi
    
    # Check for outbound network
    if grep -q 'allow_outbound_requests.*=.*false' "$CONFIG_FILE"; then
        check_pass "Outbound network requests: disabled"
    elif grep -q 'allow_outbound_requests.*=.*true' "$CONFIG_FILE"; then
        check_warn "Outbound network requests are enabled"
    fi
}

check_network_binding() {
    section_header "NETWORK BINDING"
    
    # Check if zeroclaw is running and listening
    if command -v ss &> /dev/null; then
        local listening=$(ss -tlnp 2>/dev/null | grep -i zeroclaw || true)
        if [[ -n "$listening" ]]; then
            if echo "$listening" | grep -q '0.0.0.0'; then
                check_fail "CRITICAL: ZeroClaw is listening on 0.0.0.0!"
            elif echo "$listening" | grep -q '127.0.0.1'; then
                check_pass "ZeroClaw bound to localhost only"
            else
                check_info "ZeroClaw listening: $listening"
            fi
        else
            check_info "ZeroClaw does not appear to be running (cannot verify network binding)"
        fi
    else
        check_info "'ss' command not available, skipping network binding check"
    fi
}

check_service_user() {
    section_header "SERVICE USER"
    
    # Check if zeroclaw user exists
    if id zeroclaw &>/dev/null; then
        check_pass "ZeroClaw service user exists"
        
        # Check shell
        local shell=$(getent passwd zeroclaw | cut -d: -f7)
        if [[ "$shell" == "/usr/sbin/nologin" || "$shell" == "/bin/false" ]]; then
            check_pass "Service user has no login shell: $shell"
        else
            check_warn "Service user has a login shell: $shell (should be /usr/sbin/nologin)"
        fi
    else
        check_warn "ZeroClaw service user does not exist"
    fi
    
    # Check if running as root
    if pgrep -u root zeroclaw &>/dev/null; then
        check_fail "CRITICAL: ZeroClaw is running as root!"
    fi
}

check_systemd_hardening() {
    section_header "SYSTEMD HARDENING"
    
    if [[ ! -f "$SERVICE_FILE" ]]; then
        check_warn "Systemd service file not found: $SERVICE_FILE"
        return
    fi
    
    # Check for hardening options
    local hardening_options=(
        "NoNewPrivileges=true"
        "ProtectSystem="
        "ProtectHome=true"
        "PrivateTmp=true"
        "PrivateDevices=true"
        "CapabilityBoundingSet="
        "SystemCallFilter="
    )
    
    for opt in "${hardening_options[@]}"; do
        if grep -q "$opt" "$SERVICE_FILE"; then
            check_pass "Systemd hardening: $opt"
        else
            check_warn "Missing systemd hardening: $opt"
        fi
    done
    
    # Check systemd-analyze security score if available
    if command -v systemd-analyze &>/dev/null && systemctl is-active zeroclaw &>/dev/null; then
        check_info "Running systemd-analyze security zeroclaw..."
        local score=$(systemd-analyze security zeroclaw 2>/dev/null | grep 'Overall exposure' | grep -oP '[0-9.]+' | head -1)
        if [[ -n "$score" ]]; then
            if (( $(echo "$score < 3.0" | bc -l 2>/dev/null || echo "0") )); then
                check_pass "Systemd security score: $score (SAFE)"
            elif (( $(echo "$score < 5.0" | bc -l 2>/dev/null || echo "0") )); then
                check_warn "Systemd security score: $score (MEDIUM)"
            else
                check_fail "Systemd security score: $score (EXPOSED)"
            fi
        fi
    fi
}

check_sandboxing() {
    section_header "OS-LEVEL SANDBOXING"
    
    # Check for Firejail
    if command -v firejail &>/dev/null; then
        check_pass "Firejail is installed"
        if [[ -f "/etc/firejail/zeroclaw.profile" ]]; then
            check_pass "ZeroClaw Firejail profile installed"
        else
            check_info "ZeroClaw Firejail profile not installed at /etc/firejail/zeroclaw.profile"
        fi
    else
        check_info "Firejail not installed (optional but recommended)"
    fi
    
    # Check for Bubblewrap
    if command -v bwrap &>/dev/null; then
        check_pass "Bubblewrap is installed"
    else
        check_info "Bubblewrap not installed (optional alternative to Firejail)"
    fi
    
    if ! command -v firejail &>/dev/null && ! command -v bwrap &>/dev/null; then
        check_warn "No OS-level sandboxing tools installed. Consider installing Firejail or Bubblewrap."
    fi
}

check_sensitive_files() {
    section_header "SENSITIVE FILE CHECK"
    
    local workspace="/var/lib/zeroclaw/workspace"
    
    if [[ -d "$workspace" ]]; then
        # Check for common sensitive file patterns
        local sensitive_patterns=(
            "*.key"
            "*.pem"
            "*.p12"
            "*.env"
            "*password*"
            "*secret*"
            "*credentials*"
            ".aws/*"
            ".ssh/*"
            "id_rsa*"
        )
        
        for pattern in "${sensitive_patterns[@]}"; do
            local found=$(find "$workspace" -name "$pattern" 2>/dev/null | head -5)
            if [[ -n "$found" ]]; then
                check_warn "Potentially sensitive files in workspace matching '$pattern':"
                echo "$found" | head -3 | while read -r f; do echo "         $f"; done
            fi
        done
        
        check_info "Workspace checked for sensitive files"
    else
        check_info "Workspace directory not found: $workspace"
    fi
}

check_common_misconfigs() {
    section_header "COMMON MISCONFIGURATIONS"
    
    # Check world-readable keys directory
    if [[ -d "$KEYS_DIR" ]]; then
        local world_readable=$(find "$KEYS_DIR" -perm -o+r 2>/dev/null | head -5)
        if [[ -n "$world_readable" ]]; then
            check_fail "World-readable files in keys directory!"
        else
            check_pass "No world-readable files in keys directory"
        fi
    fi
    
    # Check for .env files in config
    if [[ -f "/etc/zeroclaw/.env" ]]; then
        check_warn "Found .env file in /etc/zeroclaw/ - ensure no secrets are exposed"
    fi
    
    # Check for debug mode in config
    if grep -q 'level.*=.*"debug"' "$CONFIG_FILE" 2>/dev/null; then
        check_warn "Debug logging is enabled - may leak sensitive information"
    fi
    
    # Check for disabled TLS with network binding
    if grep -q 'bind_address.*=.*"0.0.0.0"' "$CONFIG_FILE" 2>/dev/null; then
        if grep -A3 'server.tls' "$CONFIG_FILE" 2>/dev/null | grep -q 'enabled.*=.*false'; then
            check_fail "CRITICAL: Network exposed without TLS!"
        fi
    fi
}

# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------

main() {
    echo ""
    echo -e "${CYAN}╔═══════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║          ZeroClaw Security Verification Report                            ║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${BLUE}Date:${NC} $(date)"
    echo -e "${BLUE}Host:${NC} $(hostname)"
    
    check_root
    check_file_permissions
    check_config_security
    check_network_binding
    check_service_user
    check_systemd_hardening
    check_sandboxing
    check_sensitive_files
    check_common_misconfigs
    
    # Summary
    section_header "SECURITY SUMMARY"
    
    echo ""
    echo -e "Total Checks: $TOTAL_CHECKS"
    echo -e "${GREEN}Passed:${NC}  $PASS_COUNT"
    echo -e "${YELLOW}Warnings:${NC} $WARN_COUNT"
    echo -e "${RED}Failed:${NC}   $FAIL_COUNT"
    echo ""
    
    # Calculate score
    if [[ $TOTAL_CHECKS -gt 0 ]]; then
        local score=$((PASS_COUNT * 100 / TOTAL_CHECKS))
        echo -e "Security Score: ${score}%"
        
        if [[ $score -ge 90 && $FAIL_COUNT -eq 0 ]]; then
            echo -e "${GREEN}Rating: EXCELLENT - Production ready${NC}"
        elif [[ $score -ge 75 && $FAIL_COUNT -eq 0 ]]; then
            echo -e "${GREEN}Rating: GOOD - Review warnings before production${NC}"
        elif [[ $FAIL_COUNT -gt 0 ]]; then
            echo -e "${RED}Rating: INSECURE - Fix failed checks before production${NC}"
        else
            echo -e "${YELLOW}Rating: NEEDS IMPROVEMENT - Review all findings${NC}"
        fi
    fi
    
    echo ""
    
    # Recommendations
    if [[ $FAIL_COUNT -gt 0 || $WARN_COUNT -gt 0 ]]; then
        echo -e "${CYAN}Recommendations:${NC}"
        if [[ $FAIL_COUNT -gt 0 ]]; then
            echo "  1. Address all FAIL items immediately - they are critical security issues"
        fi
        if [[ $WARN_COUNT -gt 0 ]]; then
            echo "  2. Review WARN items and determine if they're acceptable for your use case"
        fi
        echo "  3. Consider adding OS-level sandboxing (Firejail or Bubblewrap)"
        echo "  4. Enable audit logging and monitor /var/log/zeroclaw/audit.log"
        echo "  5. Review SECURITY_CHECKLIST.md before production deployment"
        echo ""
    fi
    
    # Exit code
    if [[ $FAIL_COUNT -gt 0 ]]; then
        exit 2
    elif [[ $WARN_COUNT -gt 0 ]]; then
        exit 1
    else
        exit 0
    fi
}

main "$@"
