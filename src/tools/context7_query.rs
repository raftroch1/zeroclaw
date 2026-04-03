use super::traits::{Tool, ToolResult};
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const MCP_BRIDGE_URL: &str = "http://host.docker.internal:8002";

/// Query documentation using Context7 through the MCP bridge
pub struct Context7QueryTool {
    security: Arc<SecurityPolicy>,
}

impl Context7QueryTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for Context7QueryTool {
    fn name(&self) -> &str {
        "context7_query"
    }

    fn description(&self) -> &str {
        "Query documentation from Context7. Supports many libraries like Solana, Anchor, OpenCV, Python, Anthropic, etc. Example: context7_query(query='How to create Solana account', library='solana')"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Your documentation question"
                },
                "library": {
                    "type": "string",
                    "description": "Optional library name (e.g., 'solana', 'anchor', 'python', 'anthropic')"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;

        let library = args["library"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Check rate limiting
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: too many actions in the last hour".into()),
            });
        }

        // First resolve the library if specified
        let library_id = if !library.is_empty() {
            let resolve_payload = json!({
                "library_name": library,
                "query": query
            });

            let resolve_url = format!("{}/api/mcp/context7/resolve", MCP_BRIDGE_URL);
            let client = reqwest::Client::new();

            let resolve_response = client
                .post(&resolve_url)
                .header("Content-Type", "application/json")
                .json(&resolve_payload)
                .send()
                .await;

            match resolve_response {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(result) = resp.json::<serde_json::Value>().await {
                        if let Some(lib_id) = result.get("library_id") {
                            Some(lib_id.as_str().unwrap_or("").to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None
            }
        } else {
            None
        };

        // Then query with the library_id
        let query_payload = if let Some(ref lib_id) = library_id {
            json!({
                "library_id": lib_id,
                "query": query
            })
        } else {
            json!({
                "library_id": "anthropic",
                "query": query
            })
        };

        let query_url = format!("{}/api/mcp/context7/query", MCP_BRIDGE_URL);
        let client = reqwest::Client::new();

        let query_response = client
            .post(&query_url)
            .header("Content-Type", "application/json")
            .json(&query_payload)
            .send()
            .await;

        match query_response {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(result) = resp.json::<serde_json::Value>().await {
                        let output = format!("📚 Context7 Results for '{}':\n\n{:?}", query, result);
                        return Ok(ToolResult {
                            success: true,
                            output,
                            error: None,
                        })
                    } else {
                        return Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some("Error parsing response".to_string()),
                        })
                    }
                } else {
                    let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    return Ok(ToolResult {
                        success: false,
                        output: format!("❌ Context7 query failed: {}", error_text),
                        error: Some(error_text),
                    })
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
    }
}
