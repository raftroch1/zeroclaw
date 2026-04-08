//! MCP protocol message types following JSON-RPC 2.0.
//!
//! Defines the wire format for MCP client-server communication,
//! including initialization, tool discovery, and tool invocation.

use serde::{Deserialize, Serialize};


// ── JSON-RPC 2.0 envelope ──────────────────────────────────────

/// A JSON-RPC 2.0 request message.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 response message.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

// ── MCP initialization ────────────────────────────────────────

/// MCP client capabilities sent during initialization.
#[derive(Debug, Clone, Serialize)]
pub struct McpClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<serde_json::Value>,
}

impl Default for McpClientCapabilities {
    fn default() -> Self {
        Self { roots: None }
    }
}

/// MCP client info sent during initialization.
#[derive(Debug, Clone, Serialize)]
pub struct McpClientInfo {
    pub name: String,
    pub version: String,
}

impl Default for McpClientInfo {
    fn default() -> Self {
        Self {
            name: "zeroclaw".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

/// Parameters for the `initialize` request.
#[derive(Debug, Clone, Serialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: McpClientCapabilities,
    #[serde(rename = "clientInfo")]
    pub client_info: McpClientInfo,
}

impl Default for InitializeParams {
    fn default() -> Self {
        Self {
            protocol_version: "2024-11-05".into(),
            capabilities: McpClientCapabilities::default(),
            client_info: McpClientInfo::default(),
        }
    }
}

/// Result of the `initialize` response.
#[derive(Debug, Clone, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: Option<String>,
    pub capabilities: Option<serde_json::Value>,
    #[serde(rename = "serverInfo")]
    pub server_info: Option<ServerInfo>,
}

/// Server information returned during initialization.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerInfo {
    pub name: Option<String>,
    pub version: Option<String>,
}

// ── Tool discovery ────────────────────────────────────────────

/// A tool definition as returned by `tools/list`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpToolInfo {
    /// Tool name (e.g. "brv-query", "brv-curate")
    pub name: String,
    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
    /// JSON Schema for the tool's input parameters
    #[serde(rename = "inputSchema", default)]
    pub input_schema: Option<serde_json::Value>,
}

/// Result of the `tools/list` response.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsListResult {
    pub tools: Vec<McpToolInfo>,
}

// ── Tool invocation ───────────────────────────────────────────

/// Parameters for the `tools/call` request.
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// A content item in the tool call result.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,
}

/// Result of a `tools/call` response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpToolResult {
    #[serde(default)]
    pub content: Vec<McpContent>,
    #[serde(rename = "isError", default)]
    pub is_error: bool,
}

impl McpToolResult {
    /// Extracts all text content joined with newlines.
    pub fn text_output(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| c.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ── Notifications ─────────────────────────────────────────────

/// A JSON-RPC notification (no id, no response expected).
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    /// Create the `notifications/initialized` notification.
    pub fn initialized() -> Self {
        Self {
            jsonrpc: "2.0",
            method: "notifications/initialized".into(),
            params: None,
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

/// Convert an `McpToolInfo` input schema into a ZeroClaw-compatible
/// JSON Schema for the `Tool::parameters_schema()` method.
pub fn tool_info_to_parameters_schema(info: &McpToolInfo) -> serde_json::Value {
    if let Some(ref schema) = info.input_schema {
        // MCP input schemas are typically JSON Schema objects.
        // Ensure we have "type": "object" at the top level.
        let mut s = schema.clone();
        if let Some(obj) = s.as_object_mut() {
            obj.entry("type").or_insert(serde_json::json!("object"));
            if !obj.contains_key("properties") {
                obj.insert("properties".into(), serde_json::json!({}));
            }
        }
        s
    } else {
        // No schema provided — empty object schema
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_rpc_request_serialization() {
        let req = JsonRpcRequest::new(1, "tools/list", None);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"tools/list\""));
        assert!(json.contains("\"id\":1"));
        assert!(!json.contains("\"params\""));
    }

    #[test]
    fn json_rpc_request_with_params() {
        let req = JsonRpcRequest::new(2, "tools/call", Some(serde_json::json!({"name": "test"})));
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"params\""));
        assert!(json.contains("\"name\":\"test\""));
    }

    #[test]
    fn json_rpc_response_deserialization() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}
        "#;
        let resp: JsonRpcResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.id, Some(1));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn json_rpc_error_response() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}
        "#;
        let resp: JsonRpcResponse = serde_json::from_str(raw).unwrap();
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found");
    }

    #[test]
    fn mcp_tool_result_text_output() {
        let result = McpToolResult {
            content: vec![
                McpContent {
                    content_type: "text".into(),
                    text: Some("Hello".into()),
                    data: None,
                    mime_type: None,
                },
                McpContent {
                    content_type: "text".into(),
                    text: Some("World".into()),
                    data: None,
                    mime_type: None,
                },
            ],
            is_error: false,
        };
        assert_eq!(result.text_output(), "Hello\nWorld");
    }

    #[test]
    fn tool_info_to_schema_with_input() {
        let info = McpToolInfo {
            name: "test-tool".into(),
            description: Some("A test tool".into()),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            })),
        };
        let schema = tool_info_to_parameters_schema(&info);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    #[test]
    fn tool_info_to_schema_without_input() {
        let info = McpToolInfo {
            name: "no-schema".into(),
            description: None,
            input_schema: None,
        };
        let schema = tool_info_to_parameters_schema(&info);
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
    }

    #[test]
    fn notification_initialized_format() {
        let n = JsonRpcNotification::initialized();
        let json = serde_json::to_string(&n).unwrap();
        assert!(json.contains("notifications/initialized"));
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn initialize_params_default() {
        let params = InitializeParams::default();
        assert_eq!(params.protocol_version, "2024-11-05");
        assert_eq!(params.client_info.name, "zeroclaw");
    }
}
