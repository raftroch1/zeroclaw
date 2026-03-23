//! Model Context Protocol (MCP) integration for ZeroClaw.
//!
//! This module provides MCP client functionality that connects to MCP servers
//! and exposes their tools as ZeroClaw tools. It supports both stdio and HTTP
//! transports for communicating with MCP servers.
//!
//! # Architecture
//!
//! - [`McpClient`]: Main client for connecting to and communicating with MCP servers
//! - [`McpTool`]: Wrapper that adapts MCP tools to ZeroClaw's Tool trait
//! - [`McpTransport`]: Transport layer (stdio/HTTP) for server communication
//!
//! # Example
//!
//! ```no_run
//! use zeroclaw::tools::mcp::McpClient;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut client = McpClient::new("my-mcp-server");
//!
//!     // Connect to an MCP server via stdio
//!     client.connect_stdio("node", &["/path/to/server.js"]).await?;
//!
//!     // List available tools
//!     let tools = client.list_tools().await?;
//!     println!("Available tools: {:?}", tools);
//!
//!     // Call a tool
//!     let result = client.call_tool("get_weather", serde_json::json!({
//!         "location": "San Francisco"
//!     })).await?;
//!
//!     println!("Result: {}", result);
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::process::Stdio;
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use anyhow::{Context, Result, anyhow};

use super::traits::{Tool, ToolResult};

/// MCP request/response types following the MCP protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
enum McpRequest {
    #[serde(rename = "tools/list")]
    ListTools,
    #[serde(rename = "tools/call")]
    CallTool { name: String, arguments: Option<Value> },
    #[serde(rename = "initialize")]
    Initialize {
        #[serde(rename = "protocolVersion")]
        protocol_version: String,
        capabilities: ClientCapabilities,
        #[serde(rename = "clientInfo")]
        client_info: ClientInfo,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClientCapabilities {
    #[serde(default, rename = "tools")]
    tools_option: Option<ToolCapabilities>,
    #[serde(default, rename = "resources")]
    resources_option: Option<ResourceCapabilities>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCapabilities {
    #[serde(default, rename = "listChanged")]
    list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResourceCapabilities {
    #[serde(default)]
    subscribe: bool,
    #[serde(default, rename = "listChanged")]
    list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClientInfo {
    name: String,
    version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpResponse {
    jsonrpc: String,
    #[serde(flatten)]
    content: McpResponseContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum McpResponseContent {
    Result {
        result: Value,
        id: Value,
    },
    Error {
        error: McpError,
        id: Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpError {
    code: i32,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    pub content: Vec<McpContent>,
    #[serde(default, rename = "isError")]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { uri: String },
}

/// Transport layer for MCP server communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpTransport {
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    #[serde(rename = "http")]
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

/// MCP client for connecting to and interacting with MCP servers
pub struct McpClient {
    server_name: String,
    transport: Option<McpTransport>,
    initialized: bool,
    next_id: Mutex<i64>,
    cached_tools: Mutex<Vec<ToolSchema>>,
}

impl McpClient {
    /// Create a new MCP client
    pub fn new(server_name: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
            transport: None,
            initialized: false,
            next_id: Mutex::new(1),
            cached_tools: Mutex::new(Vec::new()),
        }
    }

    /// Configure stdio transport
    pub fn with_stdio(mut self, command: impl Into<String>, args: Vec<String>) -> Self {
        self.transport = Some(McpTransport::Stdio {
            command: command.into(),
            args,
        });
        self
    }

    /// Configure HTTP transport
    pub fn with_http(mut self, url: impl Into<String>) -> Self {
        self.transport = Some(McpTransport::Http {
            url: url.into(),
            headers: HashMap::new(),
        });
        self
    }

    /// Configure HTTP transport with headers
    pub fn with_http_and_headers(mut self, url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        self.transport = Some(McpTransport::Http {
            url: url.into(),
            headers,
        });
        self
    }

    /// Initialize the MCP connection
    pub async fn initialize(&mut self) -> Result<()> {
        tracing::info!("🔧 McpClient::initialize called for {}", self.server_name);

        let transport = self.transport.as_ref()
            .ok_or_else(|| anyhow!("No transport configured"))?;

        match transport {
            McpTransport::Stdio { command, args } => {
                tracing::info!("🔧 Initializing stdio transport");
                self.initialize_stdio(command, args).await?;
            }
            McpTransport::Http { url, .. } => {
                tracing::info!("🔧 Initializing HTTP transport to: {}", url);
                self.initialize_http(url).await?;
            }
        }

        self.initialized = true;

        // Cache available tools
        tracing::info!("🔧 Caching available tools...");
        let tools = self.list_tools().await?;
        tracing::info!("✅ Cached {} tools: {:?}", tools.len(), tools.iter().map(|t| &t.name).collect::<Vec<_>>());
        *self.cached_tools.lock().await = tools;

        Ok(())
    }

    /// Initialize stdio transport
    async fn initialize_stdio(&self, command: &str, args: &[String]) -> Result<()> {
        // For now, we'll store the configuration and actually connect when making requests
        // In a full implementation, we'd maintain a persistent process
        tracing::info!("Configured MCP stdio transport: {} with args: {:?}", command, args);

        tracing::info!("Configured MCP stdio transport: {} with args: {:?}", command, args);

        Ok(())
    }

    /// Initialize HTTP transport
    async fn initialize_http(&self, url: &str) -> Result<()> {
        tracing::info!("Configured MCP HTTP transport: {}", url);
        Ok(())
    }

    /// List available tools from the MCP server
    pub async fn list_tools(&self) -> Result<Vec<ToolSchema>> {
        tracing::info!("🔧 McpClient::list_tools called");

        if !self.initialized {
            return Err(anyhow!("MCP client not initialized"));
        }

        let id = {
            let mut next = self.next_id.lock().await;
            let id = *next;
            *next += 1;
            id
        };

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/list"
        });

        tracing::info!("🔧 Sending tools/list request: {}", serde_json::to_string(&request)?);
        let response = self.send_request(&request).await?;
        tracing::info!("🔧 Got response: {}", serde_json::to_string(&response)?);

        // Parse the response
        if let Some(result) = response.get("result") {
            if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
                let parsed_tools: Result<Vec<_>> = tools.iter()
                    .map(|tool| serde_json::from_value(tool.clone())
                        .context("Failed to parse tool schema"))
                    .collect();
                let tools = parsed_tools?;
                tracing::info!("✅ Parsed {} tools", tools.len());
                return Ok(tools);
            }
        }

        tracing::warn!("⚠️ No tools found in response");
        Ok(Vec::new())
    }

    /// Call a tool on the MCP server
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpToolResult> {
        if !self.initialized {
            return Err(anyhow!("MCP client not initialized"));
        }

        let id = {
            let mut next = self.next_id.lock().await;
            let id = *next;
            *next += 1;
            id
        };

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        });

        let response = self.send_request(&request).await?;

        // Parse the response
        if let Some(result) = response.get("result") {
            return serde_json::from_value(result.clone())
                .context("Failed to parse tool result");
        }

        Err(anyhow!("Tool call failed"))
    }

    /// Send a request to the MCP server
    async fn send_request(&self, request: &Value) -> Result<Value> {
        let transport = self.transport.as_ref()
            .ok_or_else(|| anyhow!("No transport configured"))?;

        match transport {
            McpTransport::Stdio { command, args } => {
                self.send_stdio_request(command, args, request).await
            }
            McpTransport::Http { url, .. } => {
                self.send_http_request(url, request).await
            }
        }
    }

    /// Send request via stdio transport
    async fn send_stdio_request(&self, command: &str, args: &[String], request: &Value) -> Result<Value> {
        let mut child = TokioCommand::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn MCP server process")?;

        // Send request
        let request_str = serde_json::to_string(request)?;
        let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("Failed to get stdin"))?;
        stdin.write_all(request_str.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        drop(stdin);

        // Read response
        let stdout = child.stdout.ok_or_else(|| anyhow!("Failed to get stdout"))?;
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        if let Some(line) = lines.next_line().await? {
            let response: Value = serde_json::from_str(&line)?;
            return Ok(response);
        }

        Err(anyhow!("No response from MCP server"))
    }

    /// Send request via HTTP transport
    async fn send_http_request(&self, url: &str, request: &Value) -> Result<Value> {
        tracing::info!("🔧 send_http_request to: {}", url);

        let transport = self.transport.as_ref()
            .ok_or_else(|| anyhow!("No transport configured"))?;

        let headers = match transport {
            McpTransport::Http { headers, .. } => headers,
            _ => &HashMap::new(),
        };

        tracing::info!("🔧 Request headers: {:?}", headers);
        tracing::info!("🔧 Request body: {}", serde_json::to_string(request)?);

        let client = reqwest::Client::new();
        let mut http_request = client
            .post(url)
            .json(request);

        // Add authentication headers
        for (key, value) in headers {
            http_request = http_request.header(key, value);
        }

        tracing::info!("🔧 Sending HTTP request...");
        let response = http_request
            .send()
            .await
            .context("Failed to send HTTP request")?;

        tracing::info!("🔧 Got response status: {}", response.status());
        let result: Value = response.json().await?;
        tracing::info!("🔧 Got response body: {}", serde_json::to_string(&result)?);

        Ok(result)
    }

    /// Get cached tools
    pub async fn get_tools(&self) -> Vec<ToolSchema> {
        self.cached_tools.lock().await.clone()
    }
}

/// ZeroClaw tool wrapper for MCP tools
pub struct McpTool {
    mcp_client: Arc<McpClient>,
    tool_name: String,
    tool_description: String,
    tool_schema: Value,
}

impl McpTool {
    /// Create a new MCP tool wrapper
    pub fn new(
        mcp_client: Arc<McpClient>,
        tool_schema: ToolSchema,
    ) -> Self {
        Self {
            mcp_client,
            tool_name: tool_schema.name,
            tool_description: tool_schema.description,
            tool_schema: tool_schema.input_schema,
        }
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
        self.tool_schema.clone()
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        match self.mcp_client.call_tool(&self.tool_name, args).await {
            Ok(result) => {
                let output = result.content.iter()
                    .filter_map(|c| match c {
                        McpContent::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                Ok(ToolResult {
                    success: !result.is_error,
                    output,
                    error: if result.is_error { Some("Tool call failed".to_string()) } else { None },
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("MCP tool error: {}", e)),
            }),
        }
    }
}

/// Create MCP tools from an MCP server
pub async fn create_mcp_tools(
    server_name: &str,
    transport: McpTransport,
) -> Result<Vec<Arc<dyn Tool>>> {
    tracing::info!("🔧 create_mcp_tools called for server: {}", server_name);

    let mut client = McpClient::new(server_name);

    match transport {
        McpTransport::Stdio { command, args } => {
            tracing::info!("🔧 Configuring stdio transport: {} {:?}", command, args);
            client = client.with_stdio(command, args);
        }
        McpTransport::Http { url, headers } => {
            tracing::info!("🔧 Configuring HTTP transport: {} headers: {:?}", url, headers);
            client = client.with_http_and_headers(url, headers);
        }
    }

    tracing::info!("🔧 Initializing MCP client...");
    client.initialize().await?;
    tracing::info!("✅ MCP client initialized successfully");

    tracing::info!("🔧 Getting tools from MCP server...");
    let tool_schemas = client.get_tools().await;
    tracing::info!("✅ Got {} tool schemas: {:?}", tool_schemas.len(), tool_schemas.iter().map(|t| &t.name).collect::<Vec<_>>());

    let client = Arc::new(client);

    let tools: Vec<Arc<dyn Tool>> = tool_schemas.into_iter()
        .map(|schema| Arc::new(McpTool::new(client.clone(), schema)) as Arc<dyn Tool>)
        .collect();

    tracing::info!("✅ Created {} MCP tools", tools.len());
    Ok(tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mcp_client_creation() {
        let client = McpClient::new("test-server")
            .with_stdio("node", vec!["server.js".to_string()]);

        assert_eq!(client.server_name, "test-server");
    }

    #[test]
    fn tool_schema_serialization() {
        let schema = ToolSchema {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "value": { "type": "string" }
                }
            }),
        };

        let json = serde_json::to_string(&schema).unwrap();
        let parsed: ToolSchema = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, "test_tool");
        assert_eq!(parsed.description, "A test tool");
    }
}