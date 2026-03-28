use super::traits::{Tool, ToolResult};
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

const ZAI_VISION_API: &str = "https://api.z.ai/chat/completions/vision";

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
        "Analyze images using Z.AI's vision API (GLM-4.6V model). Supports image URLs and can describe images, extract text, analyze charts, understand UI designs, and more. Example: zai_vision_analyze(image_source='https://example.com/image.jpg', prompt='Describe this image in detail')"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "image_source": {
                    "type": "string",
                    "description": "URL to the image (public URL)"
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

        // Prepare chat completion request with multimodal content
        let vision_payload = json!({
            "model": "glm-4.6v",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": prompt
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": image_source
                            }
                        }
                    ]
                }
            ]
        });

        let client = reqwest::Client::new();

        let response = client
            .post(ZAI_VISION_API)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&vision_payload)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();

                // Try to parse JSON response
                match resp.json::<serde_json::Value>().await {
                    Ok(result) => {
                        // Check for Z.AI error format
                        if result.get("error").is_some() {
                            let error_info = result.get("error").unwrap();
                            let error_msg = error_info.get("message")
                                .and_then(|v| v.as_str())
                                .or_else(|| error_info.get("msg").and_then(|v| v.as_str()))
                                .unwrap_or("Unknown error");

                            // Try to get error code as string or number
                            let error_code = if let Some(s) = error_info.get("code").and_then(|v| v.as_str()) {
                                s.to_string()
                            } else if let Some(n) = error_info.get("code").and_then(|v| v.as_i64()) {
                                n.to_string()
                            } else {
                                "unknown".to_string()
                            };

                            return Ok(ToolResult {
                                success: false,
                                output: format!("❌ Z.AI Vision API Error (code {}): {}. Check that the image URL is accessible and your API key has vision capabilities enabled.", error_code, error_msg),
                                error: Some(format!("Vision API error {}: {}", error_code, error_msg)),
                            });
                        }

                        // Successful response - extract content from choices
                        if let Some(choices) = result.get("choices").and_then(|v| v.as_array()) {
                            if let Some(first_choice) = choices.first() {
                                if let Some(message) = first_choice.get("message") {
                                    if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
                                        return Ok(ToolResult {
                                            success: true,
                                            output: format!("🖼️ Vision Analysis:\n\n{}", content),
                                            error: None,
                                        });
                                    }
                                }
                            }
                        }

                        // Fallback: return raw JSON
                        let output = format!("🖼️ Vision Analysis:\n\n{}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "Error formatting results".to_string()));
                        Ok(ToolResult {
                            success: true,
                            output,
                            error: None,
                        })
                    }
                    Err(e) => {
                        // JSON parsing failed
                        Ok(ToolResult {
                            success: false,
                            output: format!("❌ Z.AI vision analysis failed: Unable to parse response (status: {}). Error: {}", status, e),
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
