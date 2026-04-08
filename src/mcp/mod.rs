//! Generic MCP (Model Context Protocol) client subsystem.
//!
//! Provides a transport-agnostic MCP client that can connect to any
//! MCP-compliant server via stdio or HTTP transports. Supports dynamic
//! tool discovery and invocation using the JSON-RPC 2.0 protocol.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌─────────────────┐
//! │  McpTool     │────▶│  McpClient   │────▶│  MCP Server     │
//! │ (Tool trait) │     │  (trait)     │     │  (stdio/http)   │
//! └─────────────┘     └──────────────┘     └─────────────────┘
//!       │                    ▲
//!       │              ┌─────┴──────┐
//!       │              │            │
//!       │     ┌────────┴───┐  ┌─────┴──────┐
//!       │     │ StdioClient│  │ HttpClient │
//!       │     └────────────┘  └────────────┘
//!       │
//!       └── Implements Tool trait, wraps a single MCP tool
//!           discovered from a server
//! ```
//!
//! # Usage
//!
//! MCP servers are configured in `config.toml` under `[mcp]`:
//!
//! ```toml
//! [mcp]
//! enabled = true
//!
//! [[mcp.servers]]
//! name = "byterover"
//! transport = "stdio"
//! command = "brv"
//! args = ["mcp"]
//! timeout_secs = 30
//! ```

pub mod client;
pub mod protocol;
pub mod stdio;
pub mod tools;

pub use client::McpClient;
#[allow(unused_imports)]
pub use protocol::{McpToolInfo, McpToolResult};
pub use stdio::StdioMcpClient;
pub use tools::McpTool;
