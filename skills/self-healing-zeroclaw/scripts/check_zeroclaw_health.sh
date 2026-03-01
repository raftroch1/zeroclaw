#!/bin/bash
# ZeroClaw Health Check Script
# Part of the self-healing-zeroclaw skill

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
ISSUES=0

echo "=== ZeroClaw Health Check ==="
echo ""

# 1. Daemon running?
echo -n "Checking daemon... "
if ps aux | grep -q "[z]eroclaw daemon"; then
    DAEMON_PID=$(ps aux | grep "[z]eroclaw daemon" | awk '{print $2}')
    echo -e "${GREEN}✅ Running (PID: $DAEMON_PID)${NC}"
else
    echo -e "${RED}❌ Not running${NC}"
    ISSUES=$((ISSUES + 1))
fi

# 2. Gateway responding?
echo -n "Checking gateway... "
if curl -s --max-time 3 http://127.0.0.1:3000/health > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Healthy${NC}"
else
    echo -e "${RED}❌ Not responding${NC}"
    ISSUES=$((ISSUES + 1))
fi

# 3. API key configured?
echo -n "Checking NVIDIA API key... "
if [ -n "${NVIDIA_API_KEY:-}" ]; then
    if echo "$NVIDIA_API_KEY" | grep -q "^nvapi-"; then
        echo -e "${GREEN}✅ Set (NVIDIA NIM)${NC}"
    else
        echo -e "${YELLOW}⚠️  Set but invalid format${NC}"
        ISSUES=$((ISSUES + 1))
    fi
else
    echo -e "${YELLOW}⚠️  Not set in environment${NC}"
    ISSUES=$((ISSUES + 1))
fi

# 4. Config file exists?
echo -n "Checking config... "
if [ -f "$HOME/.zeroclaw/config.toml" ]; then
    echo -e "${GREEN}✅ Present${NC}"
else
    echo -e "${RED}❌ Not found${NC}"
    ISSUES=$((ISSUES + 1))
fi

# 5. Telegram bot configured?
echo -n "Checking Telegram bot... "
if grep -q "telegram" "$HOME/.zeroclaw/config.toml" 2>/dev/null; then
    BOT_TOKEN=$(grep "bot_token" "$HOME/.zeroclaw/config.toml" | grep -o '":[^"]*"' | tr -d ':"')
    if [ -n "$BOT_TOKEN" ]; then
        if curl -s "https://api.telegram.org/bot${BOT_TOKEN}/getMe" | grep -q '"ok":true'; then
            echo -e "${GREEN}✅ Configured and valid${NC}"
        else
            echo -e "${RED}❌ Token invalid${NC}"
            ISSUES=$((ISSUES + 1))
        fi
    else
        echo -e "${YELLOW}⚠️  Configured but no token${NC}"
        ISSUES=$((ISSUES + 1))
    fi
else
    echo -e "${YELLOW}⚠️  Not configured${NC}"
fi

# 6. Memory database OK?
echo -n "Checking memory database... "
if ls ~/.zeroclaw/workspace/memory/*.db > /dev/null 2>&1; then
    DB_FILE=$(ls ~/.zeroclaw/workspace/memory/*.db 2>/dev/null | head -1)
    if [ -n "$DB_FILE" ]; then
        DB_SIZE=$(du -h "$DB_FILE" | cut -f1)
        echo -e "${GREEN}✅ Present ($DB_SIZE)${NC}"
    else
        echo -e "${YELLOW}⚠️  Directory exists but no database${NC}"
    fi
else
    echo -e "${YELLOW}⚠️  Not found (will be created on first run)${NC}"
fi

# 7. Workspace accessible?
echo -n "Checking workspace... "
if [ -d "$HOME/.zeroclaw/workspace" ]; then
    if [ -w "$HOME/.zeroclaw/workspace" ]; then
        echo -e "${GREEN}✅ Accessible${NC}"
    else
        echo -e "${RED}❌ Not writable${NC}"
        ISSUES=$((ISSUES + 1))
    fi
else
    echo -e "${RED}❌ Not found${NC}"
    ISSUES=$((ISSUES + 1))
fi

# 8. Log file status
echo -n "Checking log file... "
if [ -f /tmp/zeroclaw-daemon.log ]; then
    LOG_SIZE=$(du -h /tmp/zeroclaw-daemon.log 2>/dev/null | cut -f1)
    LOG_LINES=$(wc -l < /tmp/zeroclaw-daemon.log 2>/dev/null)
    echo -e "${GREEN}✅ Present ($LOG_SIZE, $LOG_LINES lines)${NC}"
else
    echo -e "${YELLOW}⚠️  Not found${NC}"
fi

# 9. Recent errors in logs?
if [ -f /tmp/zeroclaw-daemon.log ]; then
    ERROR_COUNT=$(grep -i "error\|failed\|panic" /tmp/zeroclaw-daemon.log 2>/dev/null | wc -l)
    echo -n "Checking recent errors... "
    if [ "$ERROR_COUNT" -gt 0 ]; then
        echo -e "${YELLOW}⚠️  Found $ERROR_COUNT errors (check logs for details)${NC}"
        # Show last 3 errors
        echo ""
        echo "  Last 3 errors:"
        grep -i "error\|failed\|panic" /tmp/zeroclaw-daemon.log 2>/dev/null | tail -3 | sed 's/^/    /'
    else
        echo -e "${GREEN}✅ No recent errors${NC}"
    fi
fi

# 10. Binary exists?
echo -n "Checking ZeroClaw binary... "
if [ -f "./target/release/zeroclaw" ]; then
    echo -e "${GREEN}✅ Present${NC}"
else
    echo -e "${RED}❌ Not found (run 'cargo build --release')${NC}"
    ISSUES=$((ISSUES + 1))
fi

# Summary
echo ""
echo "=== Health Check Complete ==="

if [ $ISSUES -eq 0 ]; then
    echo -e "${GREEN}✅ All checks passed! ZeroClaw is healthy.${NC}"
    exit 0
elif [ $ISSUES -le 2 ]; then
    echo -e "${YELLOW}⚠️  Found $ISSUES issue(s). Consider addressing them.${NC}"
    exit 1
else
    echo -e "${RED}❌ Found $ISSUES issues! Immediate attention required.${NC}"
    exit 2
fi
