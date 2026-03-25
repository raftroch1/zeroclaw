use super::traits::{Tool, ToolResult};
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const ZAI_WEB_SEARCH_API: &str = "https://api.z.ai/api/mcp/web_search_prime/mcp";

/// Z.AI Web Search tool - searches the web and returns results
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
        "Search the web using Z.AI's web search API. Returns results including page titles, URLs, summaries, site names, and icons. Example: zai_web_search(query='latest AI technology developments')"
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
                    "description": "Maximum number of results (default: 5, max: 20)",
                    "default": 5
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
            .unwrap_or(5)
            .min(20); // Cap at 20

        // Check rate limiting
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: too many actions in the last hour".into()),
            });
        }

        // Prepare MCP request
        let mcp_payload = json!({
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
            .header("Accept", "application/json, text/event-stream")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&mcp_payload)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(result) = resp.json::<serde_json::Value>().await {
                        // Extract search results from MCP response
                        if let Some(content) = result.get("content") {
                            if let Some(results_array) = content.as_array() {
                                let mut output = format!("🔍 Web Search Results for '{}':\n\n", query);

                                for item in results_array {
                                    if let Some(text) = item.get("text") {
                                        output.push_str(&format!("{}\n", text));
                                    }
                                }

                                return Ok(ToolResult {
                                    success: true,
                                    output,
                                    error: None,
                                });
                            }
                        }

                        // Fallback: return raw JSON
                        let output = format!("🔍 Web Search Results:\n\n{}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "Error formatting results".to_string()));
                        return Ok(ToolResult {
                            success: true,
                            output,
                            error: None,
                        });
                    }
                } else {
                    let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    return Ok(ToolResult {
                        success: false,
                        output: format!("❌ Z.AI web search failed: {}", error_text),
                        error: Some(error_text),
                    });
                }
            }
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: format!("❌ Network error: {}", e),
                    error: Some(e.to_string()),
                })
            }
        }

        Ok(ToolResult {
            success: false,
            output: "❌ Failed to parse search results".to_string(),
            error: Some("Failed to parse search results".to_string()),
        })
    }
}
