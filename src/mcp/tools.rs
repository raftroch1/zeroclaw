//! MCP tool wrapper that implements the ZeroClaw `Tool` trait.
//!
//! Each `McpTool` wraps a single MCP tool discovered from a server,
//! delegating execution to the underlying MCP client.

use super::client::McpClient;
use super::protocol::{tool_info_to_parameters_schema, McpToolInfo};
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, error};

/// A ZeroClaw tool backed by an MCP server tool.
///
/// Created during tool discovery, each `McpTool` wraps a single tool
/// from an MCP server and delegates execution via the shared MCP client.
pub struct McpTool {
    /// Shared MCP client for communication.
    client: Arc<dyn McpClient>,
    /// The tool's name (as exposed to the LLM, sanitized).
    tool_name: String,
    /// The original MCP tool name (used in `tools/call` requests).
    mcp_tool_name: String,
    /// Human-readable description.
    tool_description: String,
    /// JSON Schema for the tool's parameters.
    parameters: serde_json::Value,
    /// Server name (for logging/debugging).
    server_name: String,
}

impl McpTool {
    /// Create a new `McpTool` from an MCP tool info and client.
    ///
    /// The tool name is prefixed with the server name to avoid
    /// collisions when multiple MCP servers expose tools with the
    /// same name. The prefix can be disabled by passing `prefix = false`.
    pub fn new(client: Arc<dyn McpClient>, info: &McpToolInfo, prefix_with_server: bool) -> Self {
        let server = client.server_name().to_string();
        let tool_name = if prefix_with_server {
            // Use underscores instead of hyphens for LLM compatibility.
            let sanitized_server = server.replace('-', "_");
            let sanitized_tool = info.name.replace('-', "_");
            format!("{sanitized_server}__{sanitized_tool}")
        } else {
            info.name.replace('-', "_")
        };

        let tool_description = info
            .description
            .clone()
            .unwrap_or_else(|| format!("MCP tool '{}' from server '{}'", info.name, server));

        let parameters = tool_info_to_parameters_schema(info);

        Self {
            client,
            tool_name,
            mcp_tool_name: info.name.clone(),
            tool_description,
            parameters,
            server_name: server,
        }
    }

    /// Create tools for all tools discovered from an MCP client.
    ///
    /// Calls `list_tools()` on the client and creates one `McpTool` per
    /// discovered tool. If `prefix_with_server` is true, tool names are
    /// prefixed with the server name.
    pub async fn discover_tools(
        client: Arc<dyn McpClient>,
        prefix_with_server: bool,
    ) -> anyhow::Result<Vec<Self>> {
        let tools_info = client.list_tools().await?;
        let tools: Vec<Self> = tools_info
            .iter()
            .map(|info| Self::new(client.clone(), info, prefix_with_server))
            .collect();

        Ok(tools)
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.parameters.clone()
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        debug!(
            server = %self.server_name,
            tool = %self.tool_name,
            "Executing MCP tool"
        );

        match self.client.call_tool(&self.mcp_tool_name, args).await {
            Ok(result) => {
                let output = result.text_output();
                if result.is_error {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(if output.is_empty() {
                            format!("MCP tool '{}' returned an error", self.tool_name)
                        } else {
                            output
                        }),
                    })
                } else {
                    Ok(ToolResult {
                        success: true,
                        output,
                        error: None,
                    })
                }
            }
            Err(e) => {
                error!(
                    server = %self.server_name,
                    tool = %self.tool_name,
                    error = %e,
                    "MCP tool execution failed"
                );
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!(
                        "Failed to execute MCP tool '{}' on server '{}': {e}",
                        self.tool_name, self.server_name
                    )),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::protocol::McpToolInfo;

    // A mock MCP client for testing.
    struct MockMcpClient {
        name: String,
        tools: Vec<McpToolInfo>,
    }

    #[async_trait]
    impl McpClient for MockMcpClient {
        async fn initialize(&self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn list_tools(&self) -> anyhow::Result<Vec<McpToolInfo>> {
            Ok(self.tools.clone())
        }

        async fn call_tool(
            &self,
            tool_name: &str,
            arguments: serde_json::Value,
        ) -> anyhow::Result<super::super::protocol::McpToolResult> {
            Ok(super::super::protocol::McpToolResult {
                content: vec![super::super::protocol::McpContent {
                    content_type: "text".into(),
                    text: Some(format!("Called {tool_name} with {arguments}")),
                    data: None,
                    mime_type: None,
                }],
                is_error: false,
            })
        }

        fn server_name(&self) -> &str {
            &self.name
        }
    }

    fn mock_client() -> Arc<dyn McpClient> {
        Arc::new(MockMcpClient {
            name: "test-server".into(),
            tools: vec![
                McpToolInfo {
                    name: "brv-query".into(),
                    description: Some("Search codebase".into()),
                    input_schema: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string" }
                        },
                        "required": ["query"]
                    })),
                },
                McpToolInfo {
                    name: "brv-curate".into(),
                    description: Some("Curate results".into()),
                    input_schema: None,
                },
            ],
        })
    }

    #[test]
    fn mcp_tool_name_with_prefix() {
        let client = mock_client();
        let info = McpToolInfo {
            name: "brv-query".into(),
            description: Some("Search".into()),
            input_schema: None,
        };
        let tool = McpTool::new(client, &info, true);
        assert_eq!(tool.name(), "test_server__brv_query");
    }

    #[test]
    fn mcp_tool_name_without_prefix() {
        let client = mock_client();
        let info = McpToolInfo {
            name: "brv-query".into(),
            description: Some("Search".into()),
            input_schema: None,
        };
        let tool = McpTool::new(client, &info, false);
        assert_eq!(tool.name(), "brv_query");
    }

    #[test]
    fn mcp_tool_description() {
        let client = mock_client();
        let info = McpToolInfo {
            name: "brv-query".into(),
            description: Some("Search codebase".into()),
            input_schema: None,
        };
        let tool = McpTool::new(client, &info, false);
        assert_eq!(tool.description(), "Search codebase");
    }

    #[test]
    fn mcp_tool_default_description() {
        let client = mock_client();
        let info = McpToolInfo {
            name: "test".into(),
            description: None,
            input_schema: None,
        };
        let tool = McpTool::new(client, &info, false);
        assert!(tool.description().contains("MCP tool"));
    }

    #[test]
    fn mcp_tool_schema() {
        let client = mock_client();
        let info = McpToolInfo {
            name: "brv-query".into(),
            description: Some("Search".into()),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            })),
        };
        let tool = McpTool::new(client, &info, false);
        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    #[tokio::test]
    async fn mcp_tool_execute() {
        let client = mock_client();
        let info = McpToolInfo {
            name: "brv-query".into(),
            description: Some("Search".into()),
            input_schema: None,
        };
        let tool = McpTool::new(client, &info, false);
        let result = tool
            .execute(serde_json::json!({"query": "hello"}))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("brv_query"));
    }

    #[tokio::test]
    async fn mcp_tool_discover() {
        let client = mock_client();
        let tools = McpTool::discover_tools(client, true).await.unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name(), "test_server__brv_query");
        assert_eq!(tools[1].name(), "test_server__brv_curate");
    }
}
