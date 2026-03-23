//! Vision and image analysis tools for ZeroClaw.
//!
//! This module provides tools for analyzing images, OCR, and visual understanding
//! using various AI vision capabilities.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use anyhow::{Result, Context};

use super::traits::{Tool, ToolResult};

/// Configuration for vision tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionConfig {
    /// API endpoint for vision requests
    pub endpoint: String,

    /// API key for authentication
    pub api_key: Option<String>,

    /// Model to use for vision tasks
    pub model: String,

    /// Maximum image size in MB
    pub max_image_size_mb: usize,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.z.ai/api/coding/paas/v4/chat/completions".to_string(),
            api_key: None,
            model: "glm-4v".to_string(),
            max_image_size_mb: 10,
        }
    }
}

/// Vision analysis tool for image understanding
pub struct VisionTool {
    config: VisionConfig,
}

impl VisionTool {
    /// Create a new vision tool with default configuration
    pub fn new() -> Self {
        Self {
            config: VisionConfig::default(),
        }
    }

    /// Create a new vision tool with custom configuration
    pub fn with_config(config: VisionConfig) -> Self {
        Self { config }
    }

    /// Analyze an image from a URL or local path
    async fn analyze_image(&self, image_source: &str, prompt: &str) -> Result<String> {
        // Prepare the request payload
        let payload = json!({
            "model": self.config.model,
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

        // Make the API request
        let client = reqwest::Client::new();
        let mut request = client
            .post(&self.config.endpoint)
            .header("Content-Type", "application/json")
            .json(&payload);

        // Add API key if available
        if let Some(api_key) = &self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .await
            .context("Failed to send vision request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Vision API error: {}", error_text));
        }

        let result: Value = response.json().await.context("Failed to parse vision response")?;

        // Extract the analysis result
        let analysis = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("No analysis result");

        Ok(analysis.to_string())
    }
}

impl Default for VisionTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for VisionTool {
    fn name(&self) -> &str {
        "vision_analyze"
    }

    fn description(&self) -> &str {
        "Analyze images and provide visual understanding. Can describe scenes, read text (OCR), identify objects, and answer questions about images."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "image_source": {
                    "type": "string",
                    "description": "Image URL or local file path (e.g., 'https://example.com/image.jpg' or '/path/to/image.png')"
                },
                "prompt": {
                    "type": "string",
                    "description": "What to analyze or ask about the image (e.g., 'Describe this image', 'What text is visible?', 'Is there a person in this image?')"
                }
            },
            "required": ["image_source", "prompt"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let image_source = args["image_source"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing image_source parameter"))?;

        let prompt = args["prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing prompt parameter"))?;

        match self.analyze_image(image_source, prompt).await {
            Ok(analysis) => Ok(ToolResult {
                success: true,
                output: analysis,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Vision analysis failed: {}", e)),
            }),
        }
    }
}

/// Screenshot and immediate analysis tool
pub struct ScreenshotAnalyzeTool {
    vision_tool: VisionTool,
}

impl ScreenshotAnalyzeTool {
    /// Create a new screenshot analysis tool
    pub fn new() -> Self {
        Self {
            vision_tool: VisionTool::new(),
        }
    }

    /// Create with custom vision config
    pub fn with_config(config: VisionConfig) -> Self {
        Self {
            vision_tool: VisionTool::with_config(config),
        }
    }
}

impl Default for ScreenshotAnalyzeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ScreenshotAnalyzeTool {
    fn name(&self) -> &str {
        "screenshot_analyze"
    }

    fn description(&self) -> &str {
        "Take a screenshot and analyze it immediately. Useful for debugging UI issues, analyzing current screen state, or getting AI insights about what's displayed."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "What to analyze in the screenshot (e.g., 'Describe what's on screen', 'Read any visible text', 'What are the main elements?')"
                },
                "monitor": {
                    "type": "integer",
                    "description": "Monitor number to capture (0 for primary, 1 for secondary, etc.)",
                    "default": 0
                }
            },
            "required": ["prompt"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let prompt = args["prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing prompt parameter"))?;

        let _monitor = args["monitor"].as_i64().unwrap_or(0) as u32;

        // Take screenshot - simple implementation
        let screenshot_path = format!("/tmp/screenshot_{}.png", chrono::Utc::now().timestamp());

        // Use system screenshot command
        let screenshot_result = if cfg!(target_os = "windows") {
            // Windows: use PowerShell
            tokio::process::Command::new("powershell")
                .args(&[
                    "-Command",
                    &format!("Take-ScreenShot -File {}", screenshot_path)
                ])
                .output()
                .await
        } else if cfg!(target_os = "macos") {
            // macOS: use screencapture
            tokio::process::Command::new("screencapture")
                .args(&["-x", &screenshot_path])
                .output()
                .await
        } else {
            // Linux: try multiple methods
            tokio::process::Command::new("sh")
                .args(&[
                    "-c",
                    &format!(
                        "if command -v gnome-screenshot >/dev/null 2>&1; then \
                         gnome-screenshot -f '{}'; \
                     elif command -v scrot >/dev/null 2>&1; then \
                         scrot '{}'; \
                     elif command -v import >/dev/null 2>&1; then \
                         import -window root '{}'; \
                     else \
                         echo 'NO_SCREENSHOT_TOOL' >&2; exit 1; \
                     fi", screenshot_path, screenshot_path, screenshot_path
                    )
                ])
                .output()
                .await
        };

        match screenshot_result {
            Ok(output) if output.status.success() => {
                // Analyze the screenshot
                match self.vision_tool.analyze_image(&screenshot_path, prompt).await {
                    Ok(analysis) => Ok(ToolResult {
                        success: true,
                        output: format!("Screenshot saved to: {}\n\nAnalysis:\n{}", screenshot_path, analysis),
                        error: None,
                    }),
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: format!("Screenshot saved to: {}", screenshot_path),
                        error: Some(format!("Vision analysis failed: {}", e)),
                    }),
                }
            }
            Ok(output) => {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Screenshot failed: {}", error_msg)),
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to take screenshot: {}", e)),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vision_tool_metadata() {
        let tool = VisionTool::new();
        assert_eq!(tool.name(), "vision_analyze");
        assert!(tool.description().contains("image"));
    }

    #[test]
    fn screenshot_analyze_metadata() {
        let tool = ScreenshotAnalyzeTool::new();
        assert_eq!(tool.name(), "screenshot_analyze");
        assert!(tool.description().contains("screenshot"));
    }

    #[test]
    fn vision_config_default() {
        let config = VisionConfig::default();
        assert_eq!(config.model, "glm-4-vision");
        assert_eq!(config.max_image_size_mb, 10);
    }
}