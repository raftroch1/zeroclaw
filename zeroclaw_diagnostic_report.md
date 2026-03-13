# ZeroClaw Diagnostic Report: Tool Invocation Failure

**Date:** March 9, 2026  
**Repository:** https://github.com/raftroch1/zeroclaw  
**Branches Analyzed:** `main`, `fix/tool-specific-xml-parsing`  
**Critical Commit:** `74fe19e` (March 7, 2026 - Merge branch 'security-hardened')

---

## Executive Summary

**Root Cause Identified:** The March 7, 2026 security merge (`74fe19e`) accidentally removed the critical `chat()` method override from `OllamaProvider`, breaking tool invocation for all Ollama-hosted models including `kimi:k2` and `qwen`.

**Impact:** Tools are defined and passed to the agent loop correctly, but they never reach the Ollama API. The model receives messages without tool definitions, causing it to suggest manual curl commands instead of invoking tools.

---

## Timeline of Changes

| Date | Commit | Description | Impact |
|------|--------|-------------|--------|
| Feb 28, 2026 | `c531538` | Initial ZeroClaw fork | Working state |
| Feb 28, 2026 | `bb7280c` | Add tool execution to webhook | Working state |
| Mar 1, 2026 | `3398ca9` | XML tag parsing fix | Working state |
| **Mar 7, 2026** | **`c995be4`** | **Security hardening merge** | **BROKE TOOLS** |
| Mar 7, 2026 | `74fe19e` | Merge security-hardened to main | Broken |
| Mar 7, 2026 | `a10b42a` | Telegram chunking/tag stripping | Still broken |

---

## Root Cause Analysis

### 1. The Removed Code

In commit `c995be4` (part of the security merge), the following critical method was **removed** from `src/providers/ollama.rs`:

```rust
// THIS WAS REMOVED IN THE SECURITY MERGE
async fn chat(
    &self,
    request: crate::providers::traits::ChatRequest<'_>,
    model: &str,
    temperature: f64,
) -> anyhow::Result<ChatResponse> {
    // Convert ToolSpec to OpenAI-compatible JSON and delegate to chat_with_tools.
    if let Some(specs) = request.tools {
        if !specs.is_empty() {
            let tools: Vec<serde_json::Value> = specs
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": s.name,
                            "description": s.description,
                            "parameters": s.parameters
                        }
                    })
                })
                .collect();
            return self
                .chat_with_tools(request.messages, &tools, model, temperature)
                .await;
        }
    }

    // No tools — fall back to plain text chat.
    let text = self
        .chat_with_history(request.messages, model, temperature)
        .await?;
    Ok(ChatResponse {
        text: Some(text),
        tool_calls: vec![],
    })
}
```

### 2. Why This Breaks Tool Calling

The flow works like this:

```
Agent Loop (run_tool_call_loop)
    │
    ▼
provider.chat(request_with_tools)
    │
    ▼
[BEFORE: OllamaProvider::chat() - converts ToolSpec → JSON → chat_with_tools()]
[AFTER:  Provider::chat() default - sees supports_native_tools()=true, but...]
    │
    ▼
Default implementation checks:
  - supports_native_tools() == true? YES (Ollama returns true)
  - Has tools? YES
  - BUT: Default impl doesn't convert ToolSpec to JSON!
  - Falls through to chat_with_history() WITHOUT tools!
    │
    ▼
Ollama API receives: messages only, NO tool definitions
    │
    ▼
Model has no idea tools exist, suggests manual curl commands
```

### 3. The Default Provider::chat() Implementation

From `src/providers/traits.rs` (lines 252-295):

```rust
async fn chat(
    &self,
    request: ChatRequest<'_>,
    model: &str,
    temperature: f64,
) -> anyhow::Result<ChatResponse> {
    // If tools are provided but provider doesn't support native tools,
    // inject tool instructions into system prompt as fallback.
    if let Some(tools) = request.tools {
        if !tools.is_empty() && !self.supports_native_tools() {  // ← THIS IS THE BUG!
            // Only handles non-native providers...
        }
    }

    // Falls through here when supports_native_tools() == true
    // BUT NEVER PASSES TOOLS TO THE PROVIDER!
    let text = self
        .chat_with_history(request.messages, model, temperature)
        .await?;
    Ok(ChatResponse {
        text: Some(text),
        tool_calls: Vec::new(),
    })
}
```

**The Bug:** When `supports_native_tools() == true`, the default `chat()` assumes the provider has its own `chat()` override that handles tool conversion. But after the removal, it falls through to `chat_with_history()` which ignores tools entirely.

---

## Comparison: Working vs Broken State

### Working State (Before March 7)

```
User sends message
    ↓
Agent builds tool registry → [shell, file_read, file_write, ...]
    ↓
run_tool_call_loop() calls provider.chat(tools=[...])
    ↓
OllamaProvider::chat() converts ToolSpec → JSON
    ↓
OllamaProvider::chat_with_tools() sends to /api/chat with tools array
    ↓
Ollama receives: { messages: [...], tools: [{ type: "function", function: {...} }] }
    ↓
kimi:k2/qwen recognizes tools and calls them
    ↓
Tool results fed back → Agent loop continues
```

### Broken State (After March 7)

```
User sends message
    ↓
Agent builds tool registry → [shell, file_read, file_write, ...]  ✓ Tools exist
    ↓
run_tool_call_loop() calls provider.chat(tools=[...])  ✓ Tools passed
    ↓
Provider::chat() default implementation runs
    ↓
supports_native_tools() == true, so no prompt injection
    ↓
Falls through to chat_with_history(messages_only)  ✗ TOOLS DROPPED!
    ↓
Ollama receives: { messages: [...] }  ✗ NO TOOLS IN REQUEST!
    ↓
kimi:k2/qwen has no idea tools exist
    ↓
Model suggests: "You can use curl to do that..."
```

---

## Verification Evidence

### 1. Git Diff Shows Removal

```bash
git diff 4191028..c995be4 -- src/providers/ollama.rs | grep -A50 "async fn chat"
```

Shows the `chat()` method was completely removed (lines 657-697 in the old version).

### 2. OllamaProvider Claims Native Tool Support

```rust
// src/providers/ollama.rs
fn supports_native_tools(&self) -> bool {
    // Ollama's /api/chat supports native function-calling for capable models
    true  // ← Returns true, but chat() override is missing!
}
```

### 3. chat_with_tools() Still Works

The `chat_with_tools()` method is still intact in `OllamaProvider` (lines 525-605). The only issue is nothing calls it anymore because `chat()` was removed.

---

## Security Policy Analysis

**Not the cause.** The `SecurityPolicy::from_config()` is being used correctly:

```rust
// From user's config - this is FINE
let security = Arc::new(SecurityPolicy::from_config(&config.autonomy, workspace_dir));
```

The security policy controls:
- Which commands can be executed (allowlist)
- Path restrictions (workspace_only)
- Rate limiting

But tools are registered and passed to the agent loop correctly. The issue is purely in the Ollama provider layer losing the tools before sending to the API.

---

## Model Compatibility Notes

### kimi:k2 / kimi-k2 Tool Format

Ollama's kimi:k2 expects tools in this format:
```json
{
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_temperature",
        "description": "Get current temperature",
        "parameters": {
          "type": "object",
          "properties": { "city": { "type": "string" } },
          "required": ["city"]
        }
      }
    }
  ]
}
```

This is exactly what the **removed** `chat()` method was generating.

### qwen Tool Format

Qwen models on Ollama use the same OpenAI-compatible format. Both models work correctly when tools are included in the request—they just aren't being included anymore.

---

## Affected Files

| File | Issue |
|------|-------|
| `src/providers/ollama.rs` | Missing `chat()` override |
| `src/providers/traits.rs` | Default `chat()` doesn't handle native providers with ToolSpec |

---

## Recommended Fixes

### Fix 1: Restore chat() Override in OllamaProvider (Recommended)

Restore the removed `chat()` method. See `/home/ubuntu/zeroclaw_analysis/patches/ollama_chat_fix.patch`

### Fix 2: Update Default Provider::chat() (Alternative)

Modify the default trait implementation to handle native providers. See `/home/ubuntu/zeroclaw_analysis/patches/provider_trait_fix.patch`

---

## Test Verification

After applying the fix:

1. Tools should appear in Ollama request logs:
   ```
   Ollama request: url=... tool_count=8
   ```

2. Model should return structured tool calls, not text suggestions

3. Verify with:
   ```bash
   RUST_LOG=debug cargo run -- --channel telegram
   # Then send: "What time is it?"
   # Should see: <tool_call>{"name":"shell","arguments":{"command":"date"}}</tool_call>
   ```

---

## Appendix: Full Diff of Removed Code

```diff
-    async fn chat(
-        &self,
-        request: crate::providers::traits::ChatRequest<'_>,
-        model: &str,
-        temperature: f64,
-    ) -> anyhow::Result<ChatResponse> {
-        // Convert ToolSpec to OpenAI-compatible JSON and delegate to chat_with_tools.
-        if let Some(specs) = request.tools {
-            if !specs.is_empty() {
-                let tools: Vec<serde_json::Value> = specs
-                    .iter()
-                    .map(|s| {
-                        serde_json::json!({
-                            "type": "function",
-                            "function": {
-                                "name": s.name,
-                                "description": s.description,
-                                "parameters": s.parameters
-                            }
-                        })
-                    })
-                    .collect();
-                return self
-                    .chat_with_tools(request.messages, &tools, model, temperature)
-                    .await;
-            }
-        }
-
-        // No tools — fall back to plain text chat.
-        let text = self
-            .chat_with_history(request.messages, model, temperature)
-            .await?;
-        Ok(ChatResponse {
-            text: Some(text),
-            tool_calls: vec![],
-        })
-    }
```

---

**Report Generated:** March 9, 2026  
**Analyst:** DeepAgent
