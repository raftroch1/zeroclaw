//! Generic MCP client trait.
//!
//! Defines the interface that all MCP transport implementations must
//! satisfy. This allows the tool system to work with any MCP server
//! regardless of whether it uses stdio, HTTP, or future transports.

use super::protocol::{McpToolInfo, McpToolResult};
use async_trait::async_trait;

/// A generic MCP client that can discover and invoke tools on an MCP server.
///
/// Implementations handle transport-specific concerns (process spawning,
/// HTTP connections, etc.) while exposing a uniform interface for tool
/// discovery and invocation.
#[async_trait]
pub trait McpClient: Send + Sync {
    /// Initialize the connection to the MCP server.
    ///
    /// This should perform the MCP handshake (initialize + initialized
    /// notification) and prepare the client for tool operations.
    async fn initialize(&self) -> anyhow::Result<()>;

    /// Discover available tools from the MCP server.
    ///
    /// Sends a `tools/list` request and returns the list of tools
    /// with their names, descriptions, and input schemas.
    async fn list_tools(&self) -> anyhow::Result<Vec<McpToolInfo>>;

    /// Invoke a tool on the MCP server.
    ///
    /// Sends a `tools/call` request with the given tool name and
    /// arguments, returning the structured result.
    async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<McpToolResult>;

    /// Check if the client is connected and the server is responsive.
    async fn is_healthy(&self) -> bool {
        self.list_tools().await.is_ok()
    }

    /// Return the server name for logging/identification purposes.
    fn server_name(&self) -> &str;
}
