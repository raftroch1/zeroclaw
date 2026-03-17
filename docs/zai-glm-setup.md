# Z.AI GLM Setup

ZeroClaw supports Z.AI's GLM models through OpenAI-compatible endpoints.
This guide covers practical setup options that match current ZeroClaw provider behavior.

## Overview

ZeroClaw supports these Z.AI aliases and endpoints out of the box:

| Alias | Endpoint | Notes |
|-------|----------|-------|
| `zai` | `https://api.z.ai/api/coding/paas/v4` | Global coding endpoint |
| `zai-cn` | `https://open.bigmodel.cn/api/coding/paas/v4` | China coding endpoint |
| `glm` | `https://api.z.ai/api/paas/v4` | Standard GLM endpoint |
| `glm-cn` | `https://open.bigmodel.cn/api/paas/v4` | Standard China endpoint |

> **Note:** The `zai` aliases route to Z.AI's **coding** endpoint, while `glm` aliases
> route to the standard endpoint. Both support OpenAI-compatible tool calling.

If you need a custom base URL, see `docs/custom-providers.md`.

## Setup

### Quick Start

```bash
zeroclaw onboard \
  --provider "zai" \
  --api-key "YOUR_ZAI_API_KEY"
```

### Manual Configuration

Edit `~/.zeroclaw/config.toml`:

```toml
api_key = "YOUR_ZAI_API_KEY"
default_provider = "zai"
default_model = "glm-5"
default_temperature = 0.7
```

## Available Models

| Model | Description | Context | Best For |
|-------|-------------|---------|----------|
| `glm-5` | Flagship reasoning model | 200K | Complex agentic tasks, systems engineering |
| `glm-5-turbo` | Optimized for agent workflows | 200K (128K output) | Tool-heavy agent tasks, lower latency |
| `glm-4.7` | Strong general-purpose quality | — | Balanced quality/speed |
| `glm-4.6` | Balanced baseline | — | General use |
| `glm-4.5-air` | Lower-latency option | — | Quick responses, high throughput |

> **Recommendation:** Use `glm-5` for maximum reasoning capability, or `glm-5-turbo`
> for agent-heavy workflows that involve frequent tool calls (lower latency, optimized
> for function calling).

Model availability can vary by account/region, so use the `/models` API when in doubt.

## Tool Calling / Function Calling

GLM-5 supports **native OpenAI-format tool calling**. ZeroClaw automatically sends tools
in the standard format:

```json
{
  "tools": [{
    "type": "function",
    "function": {
      "name": "shell",
      "description": "Execute a shell command",
      "parameters": { "type": "object", "properties": { "command": { "type": "string" } }, "required": ["command"] }
    }
  }],
  "tool_choice": "auto"
}
```

GLM-5 responds with `tool_calls` in the assistant message:

```json
{
  "choices": [{
    "message": {
      "role": "assistant",
      "tool_calls": [{
        "id": "call_abc123",
        "type": "function",
        "function": {
          "name": "shell",
          "arguments": "{\"command\": \"date\"}"
        }
      }]
    }
  }]
}
```

If an endpoint does not support the `tools` parameter, ZeroClaw automatically falls back
to **prompt-guided tool calling** using `<tool_call>` XML tags.

### Troubleshooting Tool Calls

If tools aren't working:

1. **Enable debug logging:** `RUST_LOG=zeroclaw=debug` to see tool call parsing details
2. **Check the endpoint:** `zai` uses the coding endpoint; try `glm` for the standard endpoint
3. **Verify model supports tools:** `glm-5` and `glm-5-turbo` support function calling
4. **Check API response:** Tools require the model to return `tool_calls` in the response

## Verify Setup

### Test with curl

```bash
# Test OpenAI-compatible endpoint
curl -X POST "https://api.z.ai/api/coding/paas/v4/chat/completions" \
  -H "Authorization: Bearer YOUR_ZAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "glm-5",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

### Test tool calling with curl

```bash
curl -X POST "https://api.z.ai/api/coding/paas/v4/chat/completions" \
  -H "Authorization: Bearer YOUR_ZAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "glm-5",
    "messages": [{"role": "user", "content": "What is the current date?"}],
    "tools": [{
      "type": "function",
      "function": {
        "name": "shell",
        "description": "Run a shell command",
        "parameters": {"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}
      }
    }],
    "tool_choice": "auto"
  }'
```

### Test with ZeroClaw CLI

```bash
# Test agent directly
echo "Hello" | zeroclaw agent

# Check status
zeroclaw status
```

## Environment Variables

Add to your `.env` file:

```bash
# Z.AI API Key
ZAI_API_KEY=your-id.secret

# Optional generic key (used by many providers)
# API_KEY=your-id.secret
```

The key format is `id.secret` (for example: `abc123.xyz789`).

## Troubleshooting

### Rate Limiting

**Symptom:** `rate_limited` errors

**Solution:**
- Wait and retry
- Check your Z.AI plan limits
- Try `glm-4.5-air` for lower latency and higher quota tolerance

### Authentication Errors

**Symptom:** 401 or 403 errors

**Solution:**
- Verify your API key format is `id.secret`
- Check the key hasn't expired
- Ensure no extra whitespace in the key

### Model Not Found

**Symptom:** Model not available error

**Solution:**
- List available models:
```bash
curl -s "https://api.z.ai/api/coding/paas/v4/models" \
  -H "Authorization: Bearer YOUR_ZAI_API_KEY" | jq '.data[].id'
```

## Getting an API Key

1. Go to [Z.AI](https://z.ai)
2. Sign up for a Coding Plan
3. Generate an API key from the dashboard
4. Key format: `id.secret` (e.g., `abc123.xyz789`)

## Related Documentation

- [ZeroClaw README](../README.md)
- [Custom Provider Endpoints](./custom-providers.md)
- [Contributing Guide](../CONTRIBUTING.md)
