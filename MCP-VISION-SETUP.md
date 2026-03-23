# MCP Vision Server Setup Guide

This document describes how to set up and configure the MCP (Model Context Protocol) Vision Server for ZeroClaw.

## Overview

The MCP Vision Server provides vision capabilities to ZeroClaw through a separate Node.js server that runs on port 3001. This architecture allows:

- **Separation of concerns**: Vision processing runs independently
- **Protocol standardization**: Uses the MCP JSON-RPC protocol
- **Extensibility**: Easy to add more MCP servers in the future
- **Security**: Isolated vision API access

## Architecture

```
ZeroClaw (port 3000) → MCP Server (port 3001) → Vision API (Z.AI/OpenAI/etc)
```

## Prerequisites

- Node.js 18+ installed
- Z.AI API key or other vision API credentials
- `mcp-vision-server` directory in ZeroClaw root

## Installation

1. **Install MCP Server Dependencies:**
   ```bash
   cd mcp-vision-server
   npm install
   ```

2. **Configure API Key:**
   Set your Z.AI API key as environment variable:
   ```bash
   export ZAI_API_KEY="your-api-key-here"
   ```

   Or hardcode it in `mcp-vision-server/server.js` (not recommended for production).

3. **Start MCP Server:**
   ```bash
   npm run start:http
   ```

   The server will start on `http://localhost:3001`

## ZeroClaw Configuration

Add to your `~/.zeroclaw/config.toml`:

```toml
[mcp]
enabled = true

[[mcp.servers]]
name = "local-vision"
transport = "http"
url = "http://localhost:3001"
enabled = true
headers = {}

# For stdio transport (alternative):
# [[mcp.servers]]
# name = "local-vision"
# transport = "stdio"
# command = "node"
# args = ["mcp-vision-server/server.js"]
# enabled = true
```

## Configuration Options

### MCP Section (`[mcp]`)

- `enabled` (boolean): Enable/disable MCP integration (default: `false`)

### Server Configuration (`[[mcp.servers]]`)

Each server supports:

- `name` (string): Unique identifier for the server
- `enabled` (boolean): Whether this server is active
- `transport` (string): Either `"http"` or `"stdio"`

#### HTTP Transport (`transport = "http"`)

- `url` (string): HTTP endpoint URL (e.g., `"http://localhost:3001"`)
- `headers` (map): Optional HTTP headers for authentication

#### Stdio Transport (`transport = "stdio"`)

- `command` (string): Command to spawn the server
- `args` (array): Command arguments

## Available Vision Tools

Once configured, the following tool becomes available in ZeroClaw:

### `vision_analyze`

Analyzes images and provides visual understanding.

**Parameters:**
- `image_source` (string, required): Image URL or local file path
  - URLs: `"https://example.com/image.jpg"`
  - Local files: `"/path/to/image.png"` or `"C:\\Users\\...\\image.jpg"`
- `prompt` (string, required): What to analyze or ask about the image
  - Examples: `"Describe this image"`, `"What text is visible?"`, `"Is there a person?"`

**Example Usage:**
```json
{
  "image_source": "/path/to/image.jpg",
  "prompt": "Describe this image in detail, including colors, objects, and any text visible."
}
```

## Troubleshooting

### MCP Server Not Starting

**Error:** `EADDRINUSE: address already in use :::3001`

**Solution:** Another process is using port 3001. Find and stop it:
```bash
netstat -ano | grep :3001
taskkill //F //PID <process-id>
```

### API Authentication Errors

**Error:** `Request failed with status code 401`

**Solution:** Check your API key:
- Verify the Z.AI API key is correct
- Set the `ZAI_API_KEY` environment variable
- Check if the key has expired

### Unknown Model Errors

**Error:** `"Unknown Model, please check the model code."`

**Solution:** The vision model name might be incorrect. Edit `mcp-vision-server/server.js`:
```javascript
const VISION_MODEL = 'correct-model-name'; // Change this
```

### Tools Not Loading

**Symptom:** `⚠️ No MCP tools loaded` in ZeroClaw logs

**Solution:** Check MCP server is running and accessible:
```bash
curl http://localhost:3001
```

Should return MCP server info (or "Cannot GET /" which is normal).

### File Path Issues

**Symptom:** `Failed to read local file` errors

**Solution:** Use absolute paths:
- Linux/Mac: `/home/user/images/photo.jpg`
- Windows: `C:\\Users\\user\\images\\photo.jpg` or `/c/Users/user/images/photo.jpg`

## Development

### MCP Server Structure

```
mcp-vision-server/
├── package.json          # Node.js dependencies
├── server.js             # MCP protocol implementation
├── http-server.js        # HTTP wrapper for MCP
└── README.md             # Server documentation
```

### Adding New Vision APIs

To add support for other vision APIs (OpenAI, Anthropic, etc.):

1. Edit `mcp-vision-server/server.js`
2. Modify the `analyzeImage` function
3. Update the API endpoint and model configuration
4. Restart the MCP server

### Protocol Details

The MCP server implements the JSON-RPC 2.0 protocol:

**Methods:**
- `initialize`: Initialize MCP session
- `tools/list`: List available tools
- `tools/call`: Execute a tool

**Request Format:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "vision_analyze",
    "arguments": {
      "image_source": "...",
      "prompt": "..."
    }
  }
}
```

## Security Considerations

1. **API Keys**: Never commit API keys to version control
2. **File Access**: The MCP server can read any file the ZeroClaw process has access to
3. **Network Exposure**: By default, the server only listens on localhost
4. **Rate Limiting**: Consider implementing rate limiting for production use

## Performance Tips

1. **Image Size**: Large images (>5MB) may cause timeouts
2. **Concurrent Requests**: The server handles requests sequentially
3. **Caching**: Consider caching frequently analyzed images
4. **Timeout**: Default timeout is 30 seconds per request

## Future Enhancements

Potential improvements:

- Support for multiple vision APIs (OpenAI GPT-4V, Anthropic Claude, etc.)
- Batch image processing
- Image caching and deduplication
- Streaming responses for large images
- Support for video analysis
- Local vision models (Ollama, local LLMs)

## Related Files

- `src/tools/mcp.rs`: ZeroClaw MCP client implementation
- `src/tools/mod.rs`: Tool registry and MCP integration
- `src/channels/mod.rs`: Channel startup with MCP tool loading

## References

- [MCP Protocol Specification](https://modelcontextprotocol.io/)
- [Z.AI API Documentation](https://z.ai/docs)
- [ZeroClaw Architecture](docs/architecture.md)

---

**Last Updated:** 2026-03-21
**Status:** Experimental - Feature branch implementation
