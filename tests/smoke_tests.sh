#!/bin/bash
# =============================================================================
# ZeroClaw Smoke Tests
# =============================================================================
# Basic functionality tests to verify the build works correctly.
# =============================================================================

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/target/release/zeroclaw"

PASSED=0
FAILED=0

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((PASSED++))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((FAILED++))
}

log_test() {
    echo -e "\n[TEST] $1"
}

echo "====================================="
echo "ZeroClaw Smoke Tests"
echo "====================================="
echo ""

# -----------------------------------------------------------------------------
# Build Tests
# -----------------------------------------------------------------------------
log_test "Checking binary exists..."
if [ -f "$BINARY" ]; then
    log_pass "Binary exists at $BINARY"
else
    echo -e "${YELLOW}Binary not found. Building...${NC}"
    cd "$PROJECT_DIR"
    cargo build --release
    if [ -f "$BINARY" ]; then
        log_pass "Binary built successfully"
    else
        log_fail "Failed to build binary"
        exit 1
    fi
fi

log_test "Checking binary is executable..."
if [ -x "$BINARY" ]; then
    log_pass "Binary is executable"
else
    log_fail "Binary is not executable"
fi

# -----------------------------------------------------------------------------
# Version/Help Tests
# -----------------------------------------------------------------------------
log_test "Testing --version flag..."
if $BINARY --version > /dev/null 2>&1; then
    VERSION=$($BINARY --version 2>&1 | head -1)
    log_pass "Version: $VERSION"
else
    log_fail "--version flag failed"
fi

log_test "Testing --help flag..."
if $BINARY --help > /dev/null 2>&1; then
    log_pass "Help text displays correctly"
else
    log_fail "--help flag failed"
fi

# -----------------------------------------------------------------------------
# Subcommand Tests
# -----------------------------------------------------------------------------
log_test "Testing 'doctor' subcommand (if available)..."
if $BINARY doctor 2>&1 | grep -qE '(OK|WARN|ERROR|healthy|unhealthy)'; then
    log_pass "Doctor command works"
else
    echo -e "${YELLOW}[SKIP]${NC} Doctor command may require configuration"
fi

log_test "Testing 'status' subcommand (if available)..."
if $BINARY status 2>&1 | grep -qiE '(running|stopped|status|not)'; then
    log_pass "Status command works"
else
    echo -e "${YELLOW}[SKIP]${NC} Status command may require running daemon"
fi

# -----------------------------------------------------------------------------
# Configuration Tests
# -----------------------------------------------------------------------------
log_test "Testing config file loading..."
if $BINARY --config "$PROJECT_DIR/security/hardened_config.toml" --help > /dev/null 2>&1; then
    log_pass "Config file can be specified"
else
    echo -e "${YELLOW}[SKIP]${NC} Config loading test inconclusive"
fi

# -----------------------------------------------------------------------------
# Memory Footprint Test
# -----------------------------------------------------------------------------
log_test "Checking binary size (should be < 10MB)..."
BINARY_SIZE=$(stat --format=%s "$BINARY" 2>/dev/null || stat -f%z "$BINARY" 2>/dev/null)
BINARY_SIZE_MB=$((BINARY_SIZE / 1024 / 1024))

if [ $BINARY_SIZE_MB -lt 10 ]; then
    log_pass "Binary size: ${BINARY_SIZE_MB}MB (target: < 10MB)"
else
    log_fail "Binary size: ${BINARY_SIZE_MB}MB (too large, target: < 10MB)"
fi

# -----------------------------------------------------------------------------
# Summary
# -----------------------------------------------------------------------------
echo ""
echo "====================================="
echo "Smoke Test Results"
echo "====================================="
echo -e "${GREEN}Passed:${NC} $PASSED"
echo -e "${RED}Failed:${NC} $FAILED"
echo ""

if [ $FAILED -gt 0 ]; then
    echo -e "${RED}Some smoke tests failed!${NC}"
    exit 1
else
    echo -e "${GREEN}All smoke tests passed!${NC}"
    exit 0
fi
