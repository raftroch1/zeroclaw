//! Hermes Agent HTTP Gateway integration tool.
//!
//! This module implements the [`HermesGatewayTool`] which delegates tasks to a
//! Hermes Agent instance running as a separate service (typically in a sibling
//! Docker container). Communication uses the HTTP Gateway bridge pattern:
//!
//! ```text
//! ┌───────────────────┐    HTTP/REST     ┌──────────────────┐
//! │ ZeroClaw           │───────────────→ │ Hermes Gateway    │
//! │ (HermesGatewayTool)│                 │ (hermes gateway)  │
//! │                    │←───────────────│ port 8080         │
//! └───────────────────┘   JSON response  └──────────────────┘
//! ```
//!
//! # Architecture
//!
//! Hermes Agent is a Python-based multi-agent orchestration system that provides:
//! - `delegate_task()` for spawning child agents with focused goals
//! - Multiple toolsets (terminal, file, web) for child agents
//! - Configurable provider/model per delegation
//!
//! ZeroClaw communicates with Hermes via its HTTP gateway (`hermes gateway`),
//! which exposes a REST API for submitting prompts and receiving responses.
//!
//! # Configuration
//!
//! Add to `config.toml`:
//!
//! ```toml
//! [hermes]
//! enabled = true
//! gateway_url = "http://hermes:8080"   # Docker service name or host
//! timeout_secs = 300                   # 5 min for complex tasks
//! api_key = ""                         # Optional auth token
//! default_profile = ""                 # Hermes --profile name
//! shared_workspace = "/workspace"      # Shared volume mount point
//! ```
//!
//! # Docker Compose
//!
//! ```yaml
//! services:
//!   hermes:
//!     build: ./path/to/hermes-agent
//!     command: ["hermes", "gateway", "--port", "8080"]
//!     volumes:
//!       - hermes-data:/opt/data           # HERMES_HOME
//!       - shared-workspace:/workspace     # Shared with ZeroClaw
//!     environment:
//!       - HERMES_HOME=/opt/data
//!     networks:
//!       - zeroclaw-net
//! ```

use super::traits::{Tool, ToolResult};
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// Hermes Agent HTTP Gateway integration tool.
///
/// Delegates tasks to a Hermes Agent instance via its HTTP gateway API.
/// Supports single-prompt chat, task delegation with toolsets, and
/// health checking of the Hermes service.
pub struct HermesGatewayTool {
    security: Arc<SecurityPolicy>,
    /// Base URL of the Hermes gateway (e.g., "http://hermes:8080")
    gateway_url: String,
    /// Request timeout in seconds (default: 300 for complex tasks)
    timeout_secs: u64,
    /// Optional API key for Hermes gateway authentication
    api_key: Option<String>,
    /// Hermes profile name (maps to --profile flag / HERMES_HOME subdirectory)
    default_profile: Option<String>,
    /// Shared workspace path accessible to both ZeroClaw and Hermes
    shared_workspace: Option<String>,
}

impl HermesGatewayTool {
    /// Create a new HermesGatewayTool with the given configuration.
    pub fn new(security: Arc<SecurityPolicy>, gateway_url: String) -> Self {
        Self {
            security,
            gateway_url,
            timeout_secs: 300,
            api_key: None,
            default_profile: None,
            shared_workspace: None,
        }
    }

    /// Create from a full HermesConfig.
    pub fn from_config(
        security: Arc<SecurityPolicy>,
        config: &crate::config::HermesConfig,
    ) -> Self {
        Self {
            security,
            gateway_url: config.gateway_url.clone(),
            timeout_secs: config.timeout_secs,
            api_key: if config.api_key.is_empty() {
                None
            } else {
                Some(config.api_key.clone())
            },
            default_profile: if config.default_profile.is_empty() {
                None
            } else {
                Some(config.default_profile.clone())
            },
            shared_workspace: if config.shared_workspace.is_empty() {
                None
            } else {
                Some(config.shared_workspace.clone())
            },
        }
    }

    /// Build an HTTP client with the configured timeout and optional auth.
    fn build_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs));

        // Add default auth header if API key is configured
        if let Some(ref key) = self.api_key {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", key))
                    .map_err(|e| anyhow::anyhow!("Invalid API key header: {}", e))?,
            );
            builder = builder.default_headers(headers);
        }

        builder.build().map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))
    }

    /// Send a chat prompt to Hermes gateway and get a response.
    ///
    /// This is the primary operation — sends a natural language prompt to
    /// Hermes, which processes it using its agent loop and returns the result.
    async fn chat(&self, args: &serde_json::Value) -> anyhow::Result<ToolResult> {
        let prompt = args["prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'prompt' parameter for chat operation"))?;

        let model = args["model"].as_str();
        let provider = args["provider"].as_str();
        let profile = args["profile"]
            .as_str()
            .or(self.default_profile.as_deref());

        // Build the request payload for Hermes gateway
        let mut payload = json!({
            "prompt": prompt,
        });

        if let Some(m) = model {
            payload["model"] = json!(m);
        }
        if let Some(p) = provider {
            payload["provider"] = json!(p);
        }
        if let Some(prof) = profile {
            payload["profile"] = json!(prof);
        }
        if let Some(ref ws) = self.shared_workspace {
            payload["working_directory"] = json!(ws);
        }

        let url = format!("{}/chat", self.gateway_url.trim_end_matches('/'));
        let client = self.build_client()?;

        let response = match client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                if e.is_timeout() {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!(
                            "Hermes gateway request timed out after {}s. The task may be too complex \
                             or the Hermes service may be overloaded. Try breaking the task into \
                             smaller pieces or increasing timeout_secs in [hermes] config.",
                            self.timeout_secs
                        )),
                    });
                }
                if e.is_connect() {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!(
                            "Cannot connect to Hermes gateway at {}. Ensure the Hermes service is \
                             running (`hermes gateway`) and the gateway_url in [hermes] config is \
                             correct. Docker users: check that both containers are on the same \
                             network.",
                            self.gateway_url
                        )),
                    });
                }
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Hermes gateway request failed: {}", e)),
                });
            }
        };

        self.parse_response(response, "chat").await
    }

    /// Delegate a complex task to Hermes with specific toolsets and constraints.
    ///
    /// This maps to Hermes's `delegate_task()` internal function, allowing
    /// ZeroClaw to specify:
    /// - A focused goal for the child agent
    /// - Context information
    /// - Which toolsets the child agent should use
    /// - Maximum iterations
    async fn delegate_task(&self, args: &serde_json::Value) -> anyhow::Result<ToolResult> {
        let goal = args["goal"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'goal' parameter for delegate_task operation"))?;

        let context = args["context"].as_str().unwrap_or("");
        let toolsets = args["toolsets"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<String>>()
            })
            .unwrap_or_else(|| vec!["terminal".into(), "file".into(), "web".into()]);
        let max_iterations = args["max_iterations"].as_u64().unwrap_or(50);
        let profile = args["profile"]
            .as_str()
            .or(self.default_profile.as_deref());

        let mut payload = json!({
            "goal": goal,
            "context": context,
            "toolsets": toolsets,
            "max_iterations": max_iterations,
        });

        if let Some(prof) = profile {
            payload["profile"] = json!(prof);
        }
        if let Some(ref ws) = self.shared_workspace {
            payload["working_directory"] = json!(ws);
        }

        let url = format!("{}/delegate", self.gateway_url.trim_end_matches('/'));
        let client = self.build_client()?;

        let response = match client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(self.format_connection_error(&e)),
                });
            }
        };

        self.parse_response(response, "delegate_task").await
    }

    /// Check Hermes gateway health/availability.
    async fn health_check(&self) -> anyhow::Result<ToolResult> {
        let url = format!("{}/health", self.gateway_url.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let body = resp.text().await.unwrap_or_default();
                Ok(ToolResult {
                    success: true,
                    output: format!(
                        "✅ Hermes gateway is healthy at {}\nResponse: {}",
                        self.gateway_url, body
                    ),
                    error: None,
                })
            }
            Ok(resp) => Ok(ToolResult {
                success: false,
                output: format!(
                    "⚠️ Hermes gateway at {} returned status {}",
                    self.gateway_url,
                    resp.status()
                ),
                error: Some(format!("HTTP {}", resp.status())),
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: format!(
                    "❌ Cannot reach Hermes gateway at {}: {}",
                    self.gateway_url, e
                ),
                error: Some(format!("Connection failed: {}", e)),
            }),
        }
    }

    /// Parse an HTTP response from the Hermes gateway into a ToolResult.
    async fn parse_response(
        &self,
        response: reqwest::Response,
        operation: &str,
    ) -> anyhow::Result<ToolResult> {
        let status = response.status();

        if status.is_success() {
            let body = response.json::<serde_json::Value>().await.unwrap_or_else(|_| {
                json!({"error": "Failed to parse JSON response from Hermes"})
            });

            // Hermes gateway may return different response shapes depending
            // on version. We handle common patterns gracefully.
            let output = if let Some(text) = body.get("response").and_then(|v| v.as_str()) {
                text.to_string()
            } else if let Some(text) = body.get("result").and_then(|v| v.as_str()) {
                text.to_string()
            } else if let Some(text) = body.get("message").and_then(|v| v.as_str()) {
                text.to_string()
            } else if let Some(text) = body.get("output").and_then(|v| v.as_str()) {
                text.to_string()
            } else {
                // Fallback: pretty-print the full JSON response
                serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string())
            };

            let has_error = body
                .get("error")
                .map(|v| !v.is_null() && v.as_str() != Some(""))
                .unwrap_or(false);

            Ok(ToolResult {
                success: !has_error,
                output: format!("🔗 Hermes {} result:\n\n{}", operation, output),
                error: if has_error {
                    body.get("error")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                } else {
                    None
                },
            })
        } else if status.as_u16() == 401 || status.as_u16() == 403 {
            Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Hermes gateway authentication failed (HTTP {}). Check the api_key in \
                     [hermes] config.",
                    status
                )),
            })
        } else if status.as_u16() == 404 {
            Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Hermes gateway endpoint not found (HTTP 404). The '{}' operation may not \
                     be supported by this Hermes version. Check that the Hermes gateway is running \
                     with: `hermes gateway`",
                    operation
                )),
            })
        } else if status.is_server_error() {
            let body = response.text().await.unwrap_or_default();
            Ok(ToolResult {
                success: false,
                output: format!("Hermes server error: {}", body),
                error: Some(format!(
                    "Hermes gateway returned server error HTTP {}. The Hermes agent may have \
                     encountered an internal error processing the request.",
                    status
                )),
            })
        } else {
            let body = response.text().await.unwrap_or_default();
            Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Hermes gateway returned unexpected HTTP {}: {}",
                    status, body
                )),
            })
        }
    }

    /// Format a connection error with helpful context.
    fn format_connection_error(&self, error: &reqwest::Error) -> String {
        if error.is_timeout() {
            format!(
                "Hermes gateway timed out after {}s at {}. Consider increasing \
                 timeout_secs in [hermes] config or breaking the task into smaller pieces.",
                self.timeout_secs, self.gateway_url
            )
        } else if error.is_connect() {
            format!(
                "Cannot connect to Hermes gateway at {}. Verify:\n\
                 1. Hermes is running: `hermes gateway --port 8080`\n\
                 2. gateway_url in [hermes] config is correct\n\
                 3. Docker: both containers are on the same network\n\
                 4. Firewall allows traffic on the configured port",
                self.gateway_url
            )
        } else {
            format!("Hermes gateway request failed: {}", error)
        }
    }
}

#[async_trait]
impl Tool for HermesGatewayTool {
    fn name(&self) -> &str {
        "hermes_gateway"
    }

    fn description(&self) -> &str {
        "Delegate tasks to the Hermes Agent via its HTTP gateway. Hermes is a multi-agent \
         orchestration system that can execute complex tasks using terminal, file, and web tools.\n\n\
         Operations:\n\
         - chat: Send a prompt to Hermes (e.g., {\"operation\": \"chat\", \"prompt\": \"Research X and write a summary\"})\n\
         - delegate_task: Delegate a goal with specific toolsets \
           (e.g., {\"operation\": \"delegate_task\", \"goal\": \"Fix the bug in auth.py\", \
           \"context\": \"Error on line 42\", \"toolsets\": [\"terminal\", \"file\"]})\n\
         - health_check: Check if Hermes gateway is reachable \
           (e.g., {\"operation\": \"health_check\"})\n\n\
         Hermes runs in a separate container with its own agent loop, tools, and memory. \
         It shares a workspace volume with ZeroClaw for file exchange."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["chat", "delegate_task", "health_check"],
                    "description": "Operation to perform: 'chat' for general prompts, 'delegate_task' for structured task delegation, 'health_check' to verify connectivity"
                },
                "prompt": {
                    "type": "string",
                    "description": "Natural language prompt to send to Hermes (for 'chat' operation)"
                },
                "goal": {
                    "type": "string",
                    "description": "Focused goal for the delegated task (for 'delegate_task' operation)"
                },
                "context": {
                    "type": "string",
                    "description": "Additional context information for the task (for 'delegate_task' operation)"
                },
                "toolsets": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Toolsets to enable for the child agent: 'terminal', 'file', 'web' (for 'delegate_task' operation). Defaults to all three."
                },
                "max_iterations": {
                    "type": "integer",
                    "description": "Maximum agent loop iterations for the delegated task (default: 50)"
                },
                "model": {
                    "type": "string",
                    "description": "Override Hermes model for this request (e.g., 'anthropic/claude-sonnet-4-20250514')"
                },
                "provider": {
                    "type": "string",
                    "description": "Override Hermes provider for this request (e.g., 'openrouter')"
                },
                "profile": {
                    "type": "string",
                    "description": "Hermes profile name to use (overrides default_profile from config)"
                }
            },
            "required": ["operation"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        // Check rate limiting
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: too many actions in the last hour".into()),
            });
        }

        let operation = args["operation"]
            .as_str()
            .unwrap_or("chat");

        match operation {
            "chat" => self.chat(&args).await,
            "delegate_task" => self.delegate_task(&args).await,
            "health_check" => self.health_check().await,
            op => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Unknown operation '{}'. Valid operations: chat, delegate_task, health_check",
                    op
                )),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tool() -> HermesGatewayTool {
        let security = Arc::new(SecurityPolicy::default());
        HermesGatewayTool::new(security, "http://localhost:8080".into())
    }

    #[test]
    fn tool_name_is_hermes_gateway() {
        let tool = test_tool();
        assert_eq!(tool.name(), "hermes_gateway");
    }

    #[test]
    fn tool_description_is_nonempty() {
        let tool = test_tool();
        assert!(!tool.description().is_empty());
        assert!(tool.description().contains("Hermes"));
    }

    #[test]
    fn tool_schema_is_valid() {
        let tool = test_tool();
        let schema = tool.parameters_schema();
        assert!(schema.is_object());
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["operation"].is_object());
        assert!(schema["properties"]["prompt"].is_object());
        assert!(schema["properties"]["goal"].is_object());
        assert!(schema["required"].is_array());
    }

    #[test]
    fn tool_spec_generation() {
        let tool = test_tool();
        let spec = tool.spec();
        assert_eq!(spec.name, "hermes_gateway");
        assert!(spec.parameters.is_object());
    }

    #[test]
    fn from_config_sets_fields() {
        let security = Arc::new(SecurityPolicy::default());
        let config = crate::config::HermesConfig {
            enabled: true,
            gateway_url: "http://hermes:9090".into(),
            timeout_secs: 600,
            api_key: "test-key".into(),
            default_profile: "production".into(),
            shared_workspace: "/shared".into(),
        };
        let tool = HermesGatewayTool::from_config(security, &config);
        assert_eq!(tool.gateway_url, "http://hermes:9090");
        assert_eq!(tool.timeout_secs, 600);
        assert_eq!(tool.api_key.as_deref(), Some("test-key"));
        assert_eq!(tool.default_profile.as_deref(), Some("production"));
        assert_eq!(tool.shared_workspace.as_deref(), Some("/shared"));
    }

    #[test]
    fn from_config_empty_optionals() {
        let security = Arc::new(SecurityPolicy::default());
        let config = crate::config::HermesConfig::default();
        let tool = HermesGatewayTool::from_config(security, &config);
        assert!(tool.api_key.is_none());
        assert!(tool.default_profile.is_none());
        assert!(tool.shared_workspace.is_none());
    }

    #[tokio::test]
    async fn execute_unknown_operation_returns_error() {
        let tool = test_tool();
        let result = tool
            .execute(json!({ "operation": "unknown_op" }))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Unknown operation"));
    }

    #[tokio::test]
    async fn chat_without_prompt_returns_error() {
        let tool = test_tool();
        let result = tool
            .execute(json!({ "operation": "chat" }))
            .await;
        // Missing prompt should cause an error
        assert!(result.is_err() || !result.unwrap().success);
    }

    #[tokio::test]
    async fn delegate_task_without_goal_returns_error() {
        let tool = test_tool();
        let result = tool
            .execute(json!({ "operation": "delegate_task" }))
            .await;
        // Missing goal should cause an error
        assert!(result.is_err() || !result.unwrap().success);
    }

    #[tokio::test]
    async fn health_check_unreachable_returns_error() {
        let security = Arc::new(SecurityPolicy::default());
        let tool = HermesGatewayTool::new(security, "http://127.0.0.1:19999".into());
        let result = tool
            .execute(json!({ "operation": "health_check" }))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
