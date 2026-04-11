#!/bin/bash
# ByteRover MCP wrapper with automatic provider connection

# Set API key from environment
if [ -n "$ZEROCLAW_API_KEY" ]; then
    # Connect to GLM provider (Z.AI) automatically
    echo "Connecting to GLM provider..." >&2
    brv providers connect glm --api-key "$ZEROCLAW_API_KEY" >&2 || echo "Provider connection failed, continuing anyway..." >&2
fi

# Start MCP server
exec brv mcp "$@"
