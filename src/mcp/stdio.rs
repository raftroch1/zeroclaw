//! Stdio transport MCP client implementation.
//!
//! Spawns an MCP server as a child process and communicates via
//! stdin/stdout using newline-delimited JSON-RPC 2.0 messages.
//! This is the primary transport for local MCP servers like ByteRover.

use super::client::McpClient;
use super::protocol::{
    InitializeParams, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpToolInfo,
    McpToolResult, ToolCallParams, ToolsListResult,
};
use anyhow::{bail, Context};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, error, info};

/// An MCP client that communicates with a server over stdio.
///
/// The server is spawned as a child process. Communication uses
/// newline-delimited JSON-RPC 2.0 over stdin (requests) and stdout (responses).
pub struct StdioMcpClient {
    name: String,
    command: String,
    args: Vec<String>,
    env: Option<HashMap<String, String>>,
    timeout_secs: u64,
    /// Async-safe writer for the child's stdin.
    writer: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    /// Handle to the reader task.
    reader_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Handle to the child process.
    child: Arc<Mutex<Option<Child>>>,
    /// Monotonically increasing request ID counter.
    next_id: AtomicU64,
    /// Pending response channels keyed by request ID.
    pending: Arc<parking_lot::Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
}

impl StdioMcpClient {
    /// Create a new stdio MCP client.
    ///
    /// The server process is not started until [`initialize`] is called.
    pub fn new(
        name: String,
        command: String,
        args: Vec<String>,
        env: Option<HashMap<String, String>>,
        timeout_secs: u64,
    ) -> Self {
        Self {
            name,
            command,
            args,
            env,
            timeout_secs,
            writer: Arc::new(Mutex::new(None)),
            reader_handle: Arc::new(Mutex::new(None)),
            child: Arc::new(Mutex::new(None)),
            next_id: AtomicU64::new(1),
            pending: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }

    /// Create from an `MCPServerConfig`.
    pub fn from_config(config: &crate::config::MCPServerConfig) -> Self {
        Self::new(
            config.name.clone(),
            config.command.clone(),
            config.args.clone(),
            config.env.clone(),
            config.timeout_secs,
        )
    }

    /// Spawn the child process and start the reader task.
    async fn spawn(&self) -> anyhow::Result<()> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Set environment variables if provided.
        if let Some(ref env_vars) = self.env {
            for (k, v) in env_vars {
                cmd.env(k, v);
            }
        }

        info!(
            server = %self.name,
            command = %self.command,
            args = ?self.args,
            "Spawning MCP server process"
        );

        let mut child_proc = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn MCP server '{}': {} {}",
                self.name,
                self.command,
                self.args.join(" ")
            )
        })?;

        let stdin = child_proc
            .stdin
            .take()
            .context("Failed to capture MCP server stdin")?;
        let stdout = child_proc
            .stdout
            .take()
            .context("Failed to capture MCP server stdout")?;

        // Start a reader task that routes responses to pending requests.
        let pending = self.pending.clone();
        let server_name = self.name.clone();
        let handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line_buf = String::new();

            loop {
                line_buf.clear();
                match reader.read_line(&mut line_buf).await {
                    Ok(0) => {
                        debug!(server = %server_name, "MCP server stdout closed (EOF)");
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line_buf.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        // Try to parse as JSON-RPC response.
                        match serde_json::from_str::<JsonRpcResponse>(trimmed) {
                            Ok(resp) => {
                                if let Some(id) = resp.id {
                                    let mut guard = pending.lock();
                                    if let Some(tx) = guard.remove(&id) {
                                        let _ = tx.send(resp);
                                    } else {
                                        debug!(
                                            server = %server_name,
                                            id = id,
                                            "Received response for unknown request ID"
                                        );
                                    }
                                } else {
                                    // Server-sent notification (no id) — log and skip.
                                    debug!(
                                        server = %server_name,
                                        line = %trimmed,
                                        "Received server notification"
                                    );
                                }
                            }
                            Err(e) => {
                                // Not valid JSON-RPC — might be server log output.
                                debug!(
                                    server = %server_name,
                                    line = %trimmed,
                                    error = %e,
                                    "Non-JSON-RPC output from MCP server"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        error!(server = %server_name, error = %e, "Error reading from MCP server stdout");
                        break;
                    }
                }
            }
        });

        *self.writer.lock().await = Some(stdin);
        *self.reader_handle.lock().await = Some(handle);
        *self.child.lock().await = Some(child_proc);

        Ok(())
    }

    /// Send a JSON-RPC request and wait for the response.
    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<JsonRpcResponse> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, method, params);

        let (tx, rx) = oneshot::channel();

        // Register the pending response channel.
        {
            let mut guard = self.pending.lock();
            guard.insert(id, tx);
        }

        // Serialize and send the request.
        let mut json_bytes =
            serde_json::to_vec(&request).context("Failed to serialize JSON-RPC request")?;
        json_bytes.push(b'\n');

        // Write to stdin (async-safe through tokio::sync::Mutex).
        {
            let mut writer_guard = self.writer.lock().await;
            let writer = writer_guard
                .as_mut()
                .context("MCP server process not started")?;
            writer
                .write_all(&json_bytes)
                .await
                .context("Failed to write to MCP server stdin")?;
            writer
                .flush()
                .await
                .context("Failed to flush MCP server stdin")?;
        }

        // Wait for the response with timeout.
        let timeout_duration = std::time::Duration::from_secs(self.timeout_secs);
        match tokio::time::timeout(timeout_duration, rx).await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(_)) => bail!("MCP response channel closed for server '{}'", self.name),
            Err(_) => {
                // Clean up the pending entry.
                let mut guard = self.pending.lock();
                guard.remove(&id);
                bail!(
                    "MCP request timed out after {}s for server '{}'",
                    self.timeout_secs,
                    self.name
                )
            }
        }
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(&self, notification: &JsonRpcNotification) -> anyhow::Result<()> {
        let mut json_bytes =
            serde_json::to_vec(notification).context("Failed to serialize notification")?;
        json_bytes.push(b'\n');

        let mut writer_guard = self.writer.lock().await;
        let writer = writer_guard
            .as_mut()
            .context("MCP server process not started")?;
        writer
            .write_all(&json_bytes)
            .await
            .context("Failed to write notification to MCP server stdin")?;
        writer
            .flush()
            .await
            .context("Failed to flush MCP server stdin")?;

        Ok(())
    }

    /// Extract the result from a JSON-RPC response, handling errors.
    fn extract_result(response: JsonRpcResponse) -> anyhow::Result<serde_json::Value> {
        if let Some(err) = response.error {
            bail!("MCP server error: {err}");
        }
        response
            .result
            .context("MCP response missing both result and error")
    }
}

#[async_trait]
impl McpClient for StdioMcpClient {
    async fn initialize(&self) -> anyhow::Result<()> {
        // Spawn the process if not already running.
        {
            let writer = self.writer.lock().await;
            if writer.is_some() {
                return Ok(());
            }
        }

        self.spawn().await?;

        // Send initialize request.
        let params = InitializeParams::default();
        let params_json = serde_json::to_value(&params)?;
        let response = self.send_request("initialize", Some(params_json)).await?;
        let _result = Self::extract_result(response)?;

        info!(server = %self.name, "MCP server initialized successfully");

        // Send initialized notification.
        let notification = JsonRpcNotification::initialized();
        self.send_notification(&notification).await?;

        Ok(())
    }

    async fn list_tools(&self) -> anyhow::Result<Vec<McpToolInfo>> {
        let response = self.send_request("tools/list", None).await?;
        let result = Self::extract_result(response)?;
        let tools_result: ToolsListResult =
            serde_json::from_value(result).context("Failed to parse tools/list response")?;

        info!(
            server = %self.name,
            count = tools_result.tools.len(),
            "Discovered MCP tools"
        );

        for tool in &tools_result.tools {
            debug!(
                server = %self.name,
                tool = %tool.name,
                description = ?tool.description,
                "Found MCP tool"
            );
        }

        Ok(tools_result.tools)
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<McpToolResult> {
        let params = ToolCallParams {
            name: tool_name.to_string(),
            arguments: Some(arguments),
        };
        let params_json = serde_json::to_value(&params)?;
        let response = self.send_request("tools/call", Some(params_json)).await?;
        let result = Self::extract_result(response)?;
        let tool_result: McpToolResult =
            serde_json::from_value(result).context("Failed to parse tools/call response")?;

        Ok(tool_result)
    }

    fn server_name(&self) -> &str {
        &self.name
    }
}

impl Drop for StdioMcpClient {
    fn drop(&mut self) {
        // Use try_lock since we're in a sync context.
        if let Ok(mut handle_guard) = self.reader_handle.try_lock() {
            if let Some(handle) = handle_guard.take() {
                handle.abort();
            }
        }
        debug!(server = %self.name, "MCP stdio client dropped, cleaning up");
    }
}
