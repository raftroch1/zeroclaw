use super::traits::{Tool, ToolResult};
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const ZAI_WEB_SEARCH_API: &str = "https://api.z.ai/api/mcp/web_search_prime/mcp";

/// Z.AI Web Search tool - searches the web using Z.AI's MCP web search API
pub struct ZaiWebSearchTool {
    security: Arc<SecurityPolicy>,
    api_key: String,
}

impl ZaiWebSearchTool {
    pub fn new(security: Arc<SecurityPolicy>, api_key: String) -> Self {
        Self { security, api_key }
    }
}

#[async_trait]
impl Tool for ZaiWebSearchTool {
    fn name(&self) -> &str {
        "zai_web_search"
    }

    fn description(&self) -> &str {
        "Search the web using Z.AI's web search MCP API (search-prime engine). Returns LLM-optimized results including titles, URLs, summaries, site names, and icons. Example: zai_web_search(query='latest AI technology developments', max_results=5)"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 10, max: 20)",
                    "default": 10
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;

        let max_results = args["max_results"]
            .as_u64()
            .unwrap_or(10)
            .min(20); // Cap at 20

        // Check rate limiting
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: too many actions in the last hour".into()),
            });
        }

        // Prepare MCP JSON-RPC request
        let mcp_payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "webSearchPrime",
                "arguments": {
                    "query": query,
                    "maxResults": max_results
                }
            }
        });

        let client = reqwest::Client::new();

        let response = client
            .post(ZAI_WEB_SEARCH_API)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&mcp_payload)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();

                // Try to parse JSON response
                match resp.json::<serde_json::Value>().await {
                    Ok(result) => {
                        // Check for MCP error format
                        if let Some(error) = result.get("error") {
                            let error_msg = error.get("message")
                                .and_then(|v| v.as_str())
                                .or_else(|| error.get("msg").and_then(|v| v.as_str()))
                                .unwrap_or("Unknown error");

                            return Ok(ToolResult {
                                success: false,
                                output: format!("❌ Z.AI Web Search Error: {}", error_msg),
                                error: Some(format!("Web search error: {}", error_msg)),
                            });
                        }

                        // Extract content from MCP response
                        if let Some(content) = result.get("result").and_then(|v| v.get("content")) {
                            if let Some(content_array) = content.as_array() {
                                for item in content_array {
                                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                        return Ok(ToolResult {
                                            success: true,
                                            output: text.to_string(),
                                            error: None,
                                        });
                                    }
                                }
                            }
                        }

                        // Fallback: return raw JSON
                        let output = format!("🔍 Web Search Results:\n\n{}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "Error formatting results".to_string()));
                        Ok(ToolResult {
                            success: true,
                            output,
                            error: None,
                        })
                    }
                    Err(e) => {
                        Ok(ToolResult {
                            success: false,
                            output: format!("❌ Z.AI web search failed: Unable to parse response (status: {}). Error: {}", status, e),
                            error: Some(format!("Failed to parse JSON response: {}", e)),
                        })
                    }
                }
            }
            Err(e) => {
                Ok(ToolResult {
                    success: false,
                    output: format!("❌ Network error: {}", e),
                    error: Some(e.to_string()),
                })
            }
        }
    }
}
