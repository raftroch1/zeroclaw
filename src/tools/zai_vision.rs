use super::traits::{Tool, ToolResult};
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const ZAI_VISION_API: &str = "https://api.z.ai/api/mcp/vision_prime/mcp";

/// Z.AI Vision tool - analyzes images using Z.AI's vision capabilities
pub struct ZaiVisionTool {
    security: Arc<SecurityPolicy>,
    api_key: String,
}

impl ZaiVisionTool {
    pub fn new(security: Arc<SecurityPolicy>, api_key: String) -> Self {
        Self { security, api_key }
    }
}

#[async_trait]
impl Tool for ZaiVisionTool {
    fn name(&self) -> &str {
        "zai_vision_analyze"
    }

    fn description(&self) -> &str {
        "Analyze images using Z.AI's vision API. Supports local file paths and URLs. Can describe images, extract text, analyze charts, understand UI designs, and more. Example: zai_vision_analyze(image_source='path/to/image.jpg' or 'https://example.com/image.jpg', prompt='Describe this image in detail')"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "image_source": {
                    "type": "string",
                    "description": "Local file path or remote URL to the image"
                },
                "prompt": {
                    "type": "string",
                    "description": "What you want to analyze, extract, or understand from the image"
                }
            },
            "required": ["image_source", "prompt"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let image_source = args["image_source"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'image_source' parameter"))?;

        let prompt = args["prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'prompt' parameter"))?;

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
                "name": "analyzeImage",
                "arguments": {
                    "imageSource": image_source,
                    "prompt": prompt
                }
            }
        });

        let client = reqwest::Client::new();

        let response = client
            .post(ZAI_VISION_API)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&mcp_payload)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status() == 404 {
                    return Ok(ToolResult {
                        success: false,
                        output: "❌ Z.AI Vision API endpoint not found (404). The vision service may not be available or the endpoint URL has changed. Please check https://docs.z.ai/devpack/mcp/vision-mcp-server for the latest endpoint information.".to_string(),
                        error: Some("Vision API endpoint not found".to_string()),
                    });
                }

                // Try to parse JSON response
                match resp.json::<serde_json::Value>().await {
                    Ok(result) => {
                        // Check for Z.AI error format: {"code":500,"msg":"404 NOT_FOUND","success":false}
                        if result.get("success").and_then(|v| v.as_bool()) == Some(false) {
                            let error_msg = result.get("msg")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown error");

                            return Ok(ToolResult {
                                success: false,
                                output: format!("❌ Z.AI Vision API Error: {}. The vision service may require a higher subscription tier or special enablement for your API key. Check: https://docs.z.ai/devpack/mcp/vision-mcp-server", error_msg),
                                error: Some(format!("Vision API error: {}", error_msg)),
                            });
                        }

                        // Successful response - extract analysis from MCP response
                        if let Some(content) = result.get("content") {
                            if let Some(results_array) = content.as_array() {
                                let mut output = format!("🖼️ Vision Analysis for '{}':\n\n", image_source);

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
                        let output = format!("🖼️ Vision Analysis:\n\n{}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "Error formatting results".to_string()));
                        return Ok(ToolResult {
                            success: true,
                            output,
                            error: None,
                        });
                    }
                    Err(_) => {
                        // JSON parsing failed
                        return Ok(ToolResult {
                            success: false,
                            output: "❌ Z.AI vision analysis failed: Unable to parse response as JSON".to_string(),
                            error: Some("Failed to parse JSON response".to_string()),
                        });
                    }
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
            output: "❌ Failed to parse vision results".to_string(),
            error: Some("Failed to parse vision results".to_string()),
        })
    }
}
