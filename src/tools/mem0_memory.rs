use super::traits::{Tool, ToolResult};
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// Access persistent semantic memory via Mem0 Brain Surgeon MCP server
pub struct Mem0MemoryTool {
    security: Arc<SecurityPolicy>,
    base_url: String,
    timeout_secs: u64,
}

impl Mem0MemoryTool {
    pub fn new(security: Arc<SecurityPolicy>, base_url: String) -> Self {
        Self {
            security,
            base_url,
            timeout_secs: 30,
        }
    }

    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }
}

impl Mem0MemoryTool {
    pub fn with_config(security: Arc<SecurityPolicy>, base_url: String, timeout_secs: u64) -> Self {
        Self {
            security,
            base_url,
            timeout_secs,
        }
    }

    async fn search(&self, args: &serde_json::Value) -> anyhow::Result<ToolResult> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter for search operation"))?;

        let limit = args["limit"].as_u64().unwrap_or(5);

        let payload = json!({
            "query": query,
            "limit": limit
        });

        let url = format!("{}/mcp/search_memories", self.base_url);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs))
            .build()?;

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result = response.json::<serde_json::Value>().await?;
            if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                let memories = result.get("results").and_then(|v| v.as_array()).map(|v| v.as_slice()).unwrap_or(&[]);
                let output = if memories.is_empty() {
                    format!("🔍 No memories found for query: '{}'", query)
                } else {
                    let formatted: Vec<String> = memories.iter().enumerate().map(|(i, mem)| {
                        let id = mem.get("memory_id").and_then(|v| v.as_str()).unwrap_or("?");
                        let content = mem.get("memory").and_then(|v| v.as_str()).unwrap_or("");
                        let category = mem.get("category").and_then(|v| v.as_str()).unwrap_or("unknown");
                        format!("{}. [{}] {} (ID: {})", i + 1, category, content, id)
                    }).collect();
                    format!("🔍 Memories found for '{}':\n\n{}", query, formatted.join("\n"))
                };
                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            } else {
                let error_msg = result.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                Ok(ToolResult {
                    success: false,
                    output: format!("❌ Search failed: {}", error_msg),
                    error: Some(error_msg.to_string()),
                })
            }
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Ok(ToolResult {
                success: false,
                output: format!("❌ HTTP error: {}", error_text),
                error: Some(error_text),
            })
        }
    }

    async fn add(&self, args: &serde_json::Value) -> anyhow::Result<ToolResult> {
        let content = args["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' parameter for add operation"))?;

        let category = args["category"]
            .as_str()
            .unwrap_or("note")
            .to_string();

        let project = args["project"]
            .as_str()
            .unwrap_or("default")
            .to_string();

        let payload = json!({
            "content": content,
            "category": category,
            "project": project
        });

        let url = format!("{}/mcp/add_memory", self.base_url);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs))
            .build()?;

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result = response.json::<serde_json::Value>().await?;
            if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                let memory_id = result.get("memory_id").and_then(|v| v.as_str()).unwrap_or("?");
                let output = format!("✅ Memory added successfully (ID: {})\n\nCategory: {}\nContent: {}", memory_id, category, content);
                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            } else {
                let error_msg = result.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                Ok(ToolResult {
                    success: false,
                    output: format!("❌ Add failed: {}", error_msg),
                    error: Some(error_msg.to_string()),
                })
            }
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Ok(ToolResult {
                success: false,
                output: format!("❌ HTTP error: {}", error_text),
                error: Some(error_text),
            })
        }
    }

    async fn get_recent(&self, args: &serde_json::Value) -> anyhow::Result<ToolResult> {
        let limit = args["limit"].as_u64().unwrap_or(10);

        let payload = json!({
            "limit": limit
        });

        let url = format!("{}/mcp/get_recent_memories", self.base_url);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs))
            .build()?;

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result = response.json::<serde_json::Value>().await?;
            if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                let memories = result.get("results").and_then(|v| v.as_array()).map(|v| v.as_slice()).unwrap_or(&[]);
                let output = if memories.is_empty() {
                    "📋 No recent memories found".to_string()
                } else {
                    let formatted: Vec<String> = memories.iter().enumerate().map(|(i, mem)| {
                        let id = mem.get("memory_id").and_then(|v| v.as_str()).unwrap_or("?");
                        let content = mem.get("memory").and_then(|v| v.as_str()).unwrap_or("");
                        let category = mem.get("category").and_then(|v| v.as_str()).unwrap_or("unknown");
                        format!("{}. [{}] {} (ID: {})", i + 1, category, content, id)
                    }).collect();
                    format!("📋 Recent memories (last {}):\n\n{}", limit, formatted.join("\n"))
                };
                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            } else {
                let error_msg = result.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                Ok(ToolResult {
                    success: false,
                    output: format!("❌ Get recent failed: {}", error_msg),
                    error: Some(error_msg.to_string()),
                })
            }
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Ok(ToolResult {
                success: false,
                output: format!("❌ HTTP error: {}", error_text),
                error: Some(error_text),
            })
        }
    }

    async fn get_project(&self, args: &serde_json::Value) -> anyhow::Result<ToolResult> {
        let project = args["project"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'project' parameter for get_project operation"))?;

        let payload = json!({
            "project": project
        });

        let url = format!("{}/mcp/get_project_memories", self.base_url);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs))
            .build()?;

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result = response.json::<serde_json::Value>().await?;
            if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                let memories = result.get("results").and_then(|v| v.as_array()).map(|v| v.as_slice()).unwrap_or(&[]);
                let output = if memories.is_empty() {
                    format!("📁 No memories found for project: '{}'", project)
                } else {
                    let formatted: Vec<String> = memories.iter().enumerate().map(|(i, mem)| {
                        let id = mem.get("memory_id").and_then(|v| v.as_str()).unwrap_or("?");
                        let content = mem.get("memory").and_then(|v| v.as_str()).unwrap_or("");
                        let category = mem.get("category").and_then(|v| v.as_str()).unwrap_or("unknown");
                        format!("{}. [{}] {} (ID: {})", i + 1, category, content, id)
                    }).collect();
                    format!("📁 Memories for project '{}':\n\n{}", project, formatted.join("\n"))
                };
                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            } else {
                let error_msg = result.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                Ok(ToolResult {
                    success: false,
                    output: format!("❌ Get project failed: {}", error_msg),
                    error: Some(error_msg.to_string()),
                })
            }
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Ok(ToolResult {
                success: false,
                output: format!("❌ HTTP error: {}", error_text),
                error: Some(error_text),
            })
        }
    }

    async fn delete(&self, args: &serde_json::Value) -> anyhow::Result<ToolResult> {
        let memory_id = args["memory_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'memory_id' parameter for delete operation"))?;

        let payload = json!({
            "memory_id": memory_id
        });

        let url = format!("{}/mcp/delete_memory", self.base_url);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs))
            .build()?;

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result = response.json::<serde_json::Value>().await?;
            if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                Ok(ToolResult {
                    success: true,
                    output: format!("🗑️ Memory deleted successfully (ID: {})", memory_id),
                    error: None,
                })
            } else {
                let error_msg = result.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                Ok(ToolResult {
                    success: false,
                    output: format!("❌ Delete failed: {}", error_msg),
                    error: Some(error_msg.to_string()),
                })
            }
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Ok(ToolResult {
                success: false,
                output: format!("❌ HTTP error: {}", error_text),
                error: Some(error_text),
            })
        }
    }
}

#[async_trait]
impl Tool for Mem0MemoryTool {
    fn name(&self) -> &str {
        "mem0_memory"
    }

    fn description(&self) -> &str {
        "Access persistent semantic memory via Mem0 Brain Surgeon. Supports 5 operations:\n\
        - search: Find memories by query (e.g., {\"operation\": \"search\", \"query\": \"ZeroClaw security\", \"limit\": 5})\n\
        - add: Store new memory (e.g., {\"operation\": \"add\", \"content\": \"Learned X\", \"category\": \"lesson_learned\", \"project\": \"zeroclaw\"})\n\
        - get_recent: Get recent memories (e.g., {\"operation\": \"get_recent\", \"limit\": 10})\n\
        - get_project: Get memories for a project (e.g., {\"operation\": \"get_project\", \"project\": \"zeroclaw\"})\n\
        - delete: Delete memory by ID (e.g., {\"operation\": \"delete\", \"memory_id\": \"123\"})\n\
        Categories: project_context, architecture_decision, user_preference, bug_fix, lesson_learned, task_status"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["search", "add", "get_recent", "get_project", "delete"],
                    "description": "Operation to perform"
                },
                "query": {
                    "type": "string",
                    "description": "Search query (for 'search' operation)"
                },
                "content": {
                    "type": "string",
                    "description": "Memory content (for 'add' operation)"
                },
                "category": {
                    "type": "string",
                    "enum": ["project_context", "architecture_decision", "user_preference", "bug_fix", "lesson_learned", "task_status"],
                    "description": "Memory category (for 'add' operation)"
                },
                "project": {
                    "type": "string",
                    "description": "Project name (for 'add' or 'get_project' operations)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (for 'search' and 'get_recent' operations)"
                },
                "memory_id": {
                    "type": "string",
                    "description": "Memory ID to delete (for 'delete' operation)"
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

        // Route to appropriate operation
        match args["operation"].as_str().unwrap_or("search") {
            "search" => self.search(&args).await,
            "add" => self.add(&args).await,
            "get_recent" => self.get_recent(&args).await,
            "get_project" => self.get_project(&args).await,
            "delete" => self.delete(&args).await,
            op => Err(anyhow::anyhow!("Unknown operation: {}", op)),
        }
    }
}
