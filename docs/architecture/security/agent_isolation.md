# ZeroClaw Multi-Agent Isolation Architecture

**Version:** 1.0  
**Date:** 2025-04-14  
**Status:** Production Architecture  
**Audience:** System architects, security engineers, platform engineers

---

## Executive Summary

This document defines the isolation model for ZeroClaw's multi-agent execution environment. ZeroClaw supports three delegation patterns, each with distinct security boundaries: (1) **In-process delegation** (Rust, same container, tool restrictions), (2) **Container delegation** (Hermes sub-agents, separate containers, filesystem isolation), and (3) **Subprocess delegation** (ByteRover MCP, stdio child processes, workspace-scoped). Each pattern enforces strict isolation while enabling secure collaboration and data sharing.

**Key Isolation Principles:**
1. **Principle of least privilege**: Agents have minimum permissions needed
2. **Cryptographic identity**: Each agent has unique credentials/keys
3. **Filesystem namespacing**: Separate workspace directories per agent
4. **Secret compartmentalization**: Secrets not inherited; explicitly passed
5. **Resource limits**: CPU, memory, and file descriptor quotas
6. **Audit trail**: All inter-agent communication logged

---

## 1. Multi-Agent Architecture Overview

### 1.1 Deployment Model

```
┌─────────────────────────────────────────────────────────────────┐
│                        ZeroClaw Platform                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                ┌─────────────┼─────────────┐
                │             │             │
       ┌────────▼────────┐    │    ┌────────▼────────┐
       │   ZeroClaw      │    │    │   Hermes        │
       │   Container     │    │    │   Container     │
       │   (Parent)      │    │    │   (Sub-agent)   │
       │                 │    │    │                 │
       │ ┌─────────────┐ │    │    │ ┌─────────────┐ │
       │ │In-Process   │ │    │    │ │ Hermes API  │ │
       │ │Delegates    │ │    │    │ │ (sandbox)   │ │
       │ │             │ │    │    │ │             │ │
       │ │ByteRover MCP│ │    │    │ │Dependencies │ │
       │ │(stdio proc) │ │    │    │ │             │ │
       │ └─────────────┘ │    │    │ └─────────────┘ │
       │                 │    │    │                 │
       │ /workspace/     │    │    │ /workspace/     │
       │ ├── agent_a/    │    │    │ ├── agents/     │
       │ ├── agent_b/    │    │    │ │   ├── sub1/   │
       │ └── shared/     │    │    │ │   └── sub2/   │
       │                 │    │    │ └── shared/     │
       └─────────────────┘    │    └─────────────────┘
                              │
                   ┌──────────▼───────────┐
                   │  Docker Volume      │
                   │  /zeroclaw-data/    │
                   │  ├── workspace/     │
                   │  └── models/        │
                   └─────────────────────┘
```

### 1.2 Isolation Layers

| Layer | In-Process Delegates | Hermes Sub-agents | ByteRover MCP |
|-------|----------------------|-------------------|---------------|
| **Container** | Same (ZeroClaw) | Separate (Hermes) | Same (ZeroClaw) |
| **Process** | Same (Rust) | Separate (HTTP) | Child (stdio) |
| **Workspace** | Subdirectory | Container volume | Parent-bound |
| **Secrets** | Explicit pass | Environment vars | Config file |
| **Capabilities** | Limited via policy | Full app stack | Command-based |
| **Resource limits** | Rust-level | Docker limits | Process limits |
| **Network access** | Via parent | Own allowlist | Parent's rules |

---

## 2. In-Process Delegation (Rust Delegates)

### 2.1 Architecture

In-process delegation occurs within the same Rust binary, allowing ZeroClaw to spawn temporary sub-agents that handle specific tasks while maintaining tight integration with the parent's security policy.

```
┌─────────────────────────────────────────────────────┐
│          ZeroClaw Rust Runtime                      │
│                                                     │
│  ┌──────────────────────────────────────────────┐  │
│  │ Parent Agent (Agent ID: primary)             │  │
│  │ ├─ Policy: Supervised                        │  │
│  │ ├─ Workspace: /zeroclaw-data/workspace/      │  │
│  │ └─ Secrets: ALPACA_KEY, ELEVENLABS_KEY      │  │
│  │                                              │  │
│  │ Spawn Delegate:                              │  │
│  │ ┌──────────────────────────────────────────┐ │  │
│  │ │ Delegate Agent (Agent ID: task_abc123)   │ │  │
│  │ ├─ Policy: ReadOnly (restricted)           │ │  │
│  │ ├─ Workspace: .../delegates/task_abc123/   │ │  │
│  │ ├─ Max depth: 1 (no re-delegation)         │ │  │
│  │ ├─ Secrets: NONE (uses parent's APIs)      │ │  │
│  │ ├─ Timeout: 30 seconds                     │ │  │
│  │ └─ Resource limit: 256MB RAM                │ │  │
│  │                                              │ │  │
│  │    Result: { status, output, logs }        │ │  │
│  └──────────────────────────────────────────────┘ │  │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### 2.2 Delegate Agent Specification

```rust
/// In-process delegate agent definition
pub struct DelegateAgent {
    pub id: String,                          // Unique identifier
    pub parent_id: String,                   // Parent agent ID
    pub delegation_depth: usize,             // Current nesting level
    pub max_depth: usize,                    // Max allowed nesting (default: 1)
    
    pub workspace_root: PathBuf,             // Scoped workspace directory
    pub parent_workspace: PathBuf,           // Parent's workspace (for shared resources)
    
    pub policy: SecurityPolicy,              // Usually ReadOnly or Supervised
    pub allowed_commands: Vec<String>,       // Subset of parent's allowlist
    pub forbidden_commands: Vec<String>,     // Additional restrictions
    
    pub secrets: SecretContext,              // See section 2.3
    pub environment: HashMap<String, String>, // Restricted env vars
    
    pub resource_limits: ResourceLimits,     // CPU, memory, timeout
    pub created_at: SystemTime,
    pub parent_approved: bool,               // Parent must approve spawning
}

/// Resource constraints for delegate agents
pub struct ResourceLimits {
    pub max_memory_bytes: u64,              // Default: 256 MB
    pub max_wall_time: Duration,            // Default: 30 seconds
    pub max_cpu_millis: u64,                // Default: 10,000 (10 CPU-seconds)
    pub max_open_files: u32,                // Default: 256
    pub max_child_processes: u32,           // Default: 0 (no re-delegation)
}

/// Delegate secret context
pub struct SecretContext {
    pub inherited_key_ids: Vec<String>,     // IDs of secrets accessible from parent
    pub ephemeral_secrets: HashMap<String, String>, // One-time secrets for this delegate
    pub credential_scope: CredentialScope,
}

pub enum CredentialScope {
    None,                                   // No direct secret access
    ReadOnly,                               // Can read parent's secrets, not modify
    Scoped(Vec<String>),                    // Can only access specific secrets
}
```

### 2.3 Delegation Invocation

```rust
/// Spawn a delegate agent from parent
pub fn spawn_delegate(
    parent: &Agent,
    task: &DelegateTask,
    auth_service: &AuthorizationService,
) -> Result<DelegateAgent, DelegationError> {
    // Validate parent is allowed to spawn delegates
    if parent.delegation_depth >= parent.max_delegation_depth {
        return Err(DelegationError::MaxDepthExceeded);
    }
    
    // Generate unique delegate ID
    let delegate_id = format!(
        "delegate_{}_{}_{}",
        parent.id,
        chrono::Utc::now().timestamp_millis(),
        rand::random::<u32>()
    );
    
    // Create scoped workspace
    let delegate_workspace = parent.workspace_root.join("delegates").join(&delegate_id);
    std::fs::create_dir_all(&delegate_workspace)?;
    
    // Restrict command allowlist if needed
    let allowed_commands = if let Some(ref cmds) = task.restricted_commands {
        parent.allowed_commands.iter()
            .filter(|cmd| cmds.contains(cmd))
            .cloned()
            .collect()
    } else {
        parent.allowed_commands.clone()
    };
    
    // Create restricted secret context
    let secrets = SecretContext {
        inherited_key_ids: task.required_secrets.clone(),
        ephemeral_secrets: Default::default(),
        credential_scope: task.credential_scope.clone(),
    };
    
    // Spawn the delegate
    let delegate = DelegateAgent {
        id: delegate_id.clone(),
        parent_id: parent.id.clone(),
        delegation_depth: parent.delegation_depth + 1,
        max_depth: 0,  // Delegates cannot re-delegate by default
        
        workspace_root: delegate_workspace,
        parent_workspace: parent.workspace_root.clone(),
        
        policy: SecurityPolicy::ReadOnly,  // Most restrictive by default
        allowed_commands,
        forbidden_commands: task.forbidden_commands.clone(),
        
        secrets,
        environment: restrict_environment(&parent.environment),
        
        resource_limits: ResourceLimits {
            max_memory_bytes: 256 * 1024 * 1024,  // 256 MB
            max_wall_time: Duration::from_secs(30),
            max_cpu_millis: 10_000,
            max_open_files: 256,
            max_child_processes: 0,  // No re-delegation
        },
        
        created_at: SystemTime::now(),
        parent_approved: true,
    };
    
    // Log delegation event
    audit_log(AuditEvent::DelegateSpawned {
        delegate_id: delegate_id.clone(),
        parent_id: parent.id.clone(),
        workspace: delegate.workspace_root.clone(),
        policy: delegate.policy.clone(),
        timestamp: SystemTime::now(),
    });
    
    Ok(delegate)
}

pub struct DelegateTask {
    pub description: String,
    pub commands_to_run: Vec<String>,
    pub required_secrets: Vec<String>,     // e.g., ["ELEVENLABS_KEY"]
    pub restricted_commands: Option<Vec<String>>,  // Subset of parent's allowlist
    pub forbidden_commands: Vec<String>,   // Additional blocks
    pub credential_scope: CredentialScope,
}

pub enum DelegationError {
    MaxDepthExceeded,
    UnauthorizedDelegation(String),
    WorkspaceCreationFailed,
    SecretAccessDenied(String),
}
```

### 2.4 Secret Access Control for Delegates

```rust
/// Check if delegate can access a secret
pub fn can_delegate_access_secret(
    delegate: &DelegateAgent,
    secret_id: &str,
    secret_service: &SecretService,
) -> Result<bool, DelegationError> {
    // Check credential scope
    match &delegate.secrets.credential_scope {
        CredentialScope::None => {
            return Ok(false);  // No secret access allowed
        }
        CredentialScope::ReadOnly => {
            // Can read, but only if secret_id is in inherited_key_ids
            if delegate.secrets.inherited_key_ids.contains(&secret_id.to_string()) {
                return Ok(true);
            } else {
                return Ok(false);
            }
        }
        CredentialScope::Scoped(allowed) => {
            // Can only access explicitly listed secrets
            return Ok(allowed.contains(&secret_id.to_string()));
        }
    }
}

/// Retrieve secret for delegate (with audit)
pub fn get_secret_for_delegate(
    delegate: &DelegateAgent,
    secret_id: &str,
    secret_service: &SecretService,
) -> Result<String, DelegationError> {
    // Check access permission
    if !can_delegate_access_secret(delegate, secret_id, secret_service)? {
        audit_log(AuditEvent::DelegateSecretAccessDenied {
            delegate_id: delegate.id.clone(),
            secret_id: secret_id.to_string(),
            reason: "Not in credential scope".into(),
        });
        return Err(DelegationError::SecretAccessDenied(
            format!("Delegate {} cannot access secret {}", delegate.id, secret_id)
        ));
    }
    
    // Retrieve secret
    let secret = secret_service.get_secret(secret_id)?;
    
    // Audit the access
    audit_log(AuditEvent::DelegateSecretAccessed {
        delegate_id: delegate.id.clone(),
        parent_id: delegate.parent_id.clone(),
        secret_id: secret_id.to_string(),
        timestamp: SystemTime::now(),
    });
    
    Ok(secret)
}

/// Restrict environment variables for delegate
fn restrict_environment(parent_env: &HashMap<String, String>) -> HashMap<String, String> {
    let mut restricted = HashMap::new();
    
    // Whitelist safe environment variables
    let safe_vars = vec![
        "PATH",              // Restricted to workspace tools
        "HOME",              // Scoped to workspace
        "RUST_LOG",          // Logging control
        "TZ",                // Timezone
        "LANG",              // Language/locale
    ];
    
    for var in safe_vars {
        if let Some(value) = parent_env.get(var) {
            restricted.insert(var.to_string(), value.clone());
        }
    }
    
    // Override with restricted values
    restricted.insert("PATH".to_string(), "/usr/local/bin:/usr/bin:/bin".to_string());
    restricted.insert("HOME".to_string(), "/workspace/delegates/home".to_string());
    
    // Block dangerous variables
    let blocked_vars = vec![
        "LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT",
        "BASH_ENV", "ENV",
        "ALPACA_API_KEY", "ALPACA_SECRET_KEY",  // Secrets not inherited via env
        "AWS_*", "AZURE_*", "GCP_*",             // Cloud credentials
    ];
    
    for pattern in blocked_vars {
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len()-1];
            restricted.retain(|k, _| !k.starts_with(prefix));
        } else {
            restricted.remove(pattern);
        }
    }
    
    restricted
}

/// Monitor delegate resource usage
pub fn monitor_delegate_resources(delegate: &DelegateAgent) -> ResourceUsage {
    // Implementation would use /proc to monitor:
    // - Memory (RSS, VSZ)
    // - CPU time
    // - File descriptors
    // - Child process count
    
    ResourceUsage {
        memory_used_bytes: 0,  // Get from /proc/self/status
        cpu_millis_used: 0,    // Get from /proc/self/stat
        open_files: 0,         // Count from /proc/self/fd
        child_processes: 0,    // Count from /proc/self/task
        wall_time_elapsed: Duration::from_secs(0),
    }
}

pub struct ResourceUsage {
    pub memory_used_bytes: u64,
    pub cpu_millis_used: u64,
    pub open_files: u32,
    pub child_processes: u32,
    pub wall_time_elapsed: Duration,
}
```

### 2.5 Delegate Execution & Cleanup

```rust
/// Execute command as delegate agent
pub async fn execute_as_delegate(
    delegate: &DelegateAgent,
    command: &str,
    context: &OperationContext,
    config: &SecurityConfig,
) -> Result<DelegateExecutionResult, DelegationError> {
    // Validate command is in delegate's allowlist
    let base_cmd = command.split_whitespace().next().unwrap_or("");
    if !delegate.allowed_commands.contains(&base_cmd.to_string()) {
        return Err(DelegationError::UnauthorizedDelegation(
            format!("Command {} not in delegate allowlist", base_cmd)
        ));
    }
    
    // Check if command is in forbidden list
    if delegate.forbidden_commands.contains(&base_cmd.to_string()) {
        return Err(DelegationError::UnauthorizedDelegation(
            format!("Command {} is forbidden for delegate", base_cmd)
        ));
    }
    
    // Set up execution environment
    let mut cmd = tokio::process::Command::new("/bin/bash");
    cmd.arg("-c").arg(command);
    
    // Set working directory to delegate's workspace
    cmd.current_dir(&delegate.workspace_root);
    
    // Set restricted environment
    cmd.env_clear();
    for (key, value) in &delegate.environment {
        cmd.env(key, value);
    }
    
    // Apply resource limits using libc
    cmd.pre_exec(|| {
        // Memory limit (soft and hard)
        unsafe {
            libc::setrlimit(libc::RLIMIT_AS, &libc::rlimit {
                rlim_cur: 256 * 1024 * 1024,  // 256 MB soft
                rlim_max: 512 * 1024 * 1024,  // 512 MB hard
            });
            
            // CPU time limit
            libc::setrlimit(libc::RLIMIT_CPU, &libc::rlimit {
                rlim_cur: 30,  // 30 seconds soft
                rlim_max: 60,  // 60 seconds hard
            });
            
            // Open files limit
            libc::setrlimit(libc::RLIMIT_NOFILE, &libc::rlimit {
                rlim_cur: 256,
                rlim_max: 512,
            });
        }
        Ok(())
    });
    
    // Start with timeout
    let timeout = delegate.resource_limits.max_wall_time;
    let output = tokio::time::timeout(timeout, async {
        cmd.output().await
    }).await
        .map_err(|_| DelegationError::ExecutionTimeout)?
        .map_err(|e| DelegationError::ExecutionFailed(e.to_string()))?;
    
    // Check resource usage
    let usage = monitor_delegate_resources(delegate);
    if usage.memory_used_bytes > delegate.resource_limits.max_memory_bytes {
        return Err(DelegationError::ResourceExceeded("Memory limit exceeded".into()));
    }
    
    // Log execution result
    audit_log(AuditEvent::DelegateExecuted {
        delegate_id: delegate.id.clone(),
        command: command.to_string(),
        exit_code: output.status.code().unwrap_or(-1),
        resource_usage: usage.clone(),
        timestamp: SystemTime::now(),
    });
    
    Ok(DelegateExecutionResult {
        status_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        resource_usage: usage,
    })
}

pub struct DelegateExecutionResult {
    pub status_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub resource_usage: ResourceUsage,
}

/// Cleanup delegate after execution
pub fn cleanup_delegate(delegate: &DelegateAgent) -> Result<(), DelegationError> {
    // Remove delegate's workspace directory
    std::fs::remove_dir_all(&delegate.workspace_root)
        .map_err(|e| DelegationError::CleanupFailed(e.to_string()))?;
    
    // Revoke any ephemeral secrets
    // ...
    
    // Log cleanup
    audit_log(AuditEvent::DelegateTerminated {
        delegate_id: delegate.id.clone(),
        parent_id: delegate.parent_id.clone(),
        timestamp: SystemTime::now(),
    });
    
    Ok(())
}
```

---

## 3. Container Delegation (Hermes Sub-agents)

### 3.1 Architecture

Hermes sub-agents run in separate containers orchestrated by the Hermes framework. Each sub-agent has its own filesystem, network namespace, and security context.

```
┌────────────────────────────────────────────────────────────────┐
│                   Docker Host                                  │
│                                                                │
│  ┌─────────────────────────┐    ┌──────────────────────────┐  │
│  │  ZeroClaw Container     │    │  Hermes Container        │  │
│  │  (Primary Agent)        │    │  (Agent Orchestrator)    │  │
│  │                         │    │                          │  │
│  │ /zeroclaw-data/         │    │ /hermes/                │  │
│  │ ├── workspace/          │    │ ├── config/             │  │
│  │ └── models/             │    │ ├── agents/             │  │
│  │                         │    │ │  ├── agent_1/         │  │
│  │ HTTP POST to Hermes API │    │ │  ├── agent_2/         │  │
│  │ ─────────────────────→  │    │ │  └── agent_n/         │  │
│  │ spawn sub-agent         │    │ └── shared/             │  │
│  │                         │    │                          │  │
│  │ ←─────────────────────  │    │ Spawns: docker run ...  │  │
│  │ { sub_agent_id, host } │    │  hermes-worker:latest   │  │
│  │                         │    │                          │  │
│  └────────┬────────────────┘    └──────────┬───────────────┘  │
│           │                                 │                  │
│           │         Docker Network          │                  │
│           └─────────────────────────────────┘                  │
│                     zeroclaw-internal                          │
│                  (172.25.0.0/16)                              │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │  Sub-agent Container (On-demand)                      │   │
│  │                                                        │   │
│  │  Image: hermes-worker:latest                          │   │
│  │  /workspace/agents/sub_agent_xyz/                     │   │
│  │  ├── input/                                           │   │
│  │  ├── output/                                          │   │
│  │  ├── tmp/                                             │   │
│  │  └── env (restricted)                                 │   │
│  │                                                        │   │
│  │  Read-only mounts:                                    │   │
│  │  └─ /zeroclaw-data/workspace/shared/:ro               │   │
│  │                                                        │   │
│  │  Capabilities: NONE (dropped)                         │   │
│  │  Secrets: None (passed via HTTP)                      │   │
│  │  Timeout: 300 seconds                                 │   │
│  │  Memory: 512 MB limit                                 │   │
│  │                                                        │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### 3.2 Sub-agent Spawn API

```rust
/// Request to spawn a Hermes sub-agent
pub struct HermesDelegateRequest {
    pub parent_agent_id: String,           // ZeroClaw agent spawning the sub-agent
    pub sub_agent_name: String,            // e.g., "text_processor_1"
    pub image: String,                     // Docker image to run
    pub command: Vec<String>,              // Entrypoint command
    pub environment: HashMap<String, String>,  // Restricted env vars
    pub resource_limits: HermesResourceLimits,
    pub secrets: Vec<SecretReference>,     // Secrets to pass (by ID, not value)
    pub input_data: Option<Vec<u8>>,       // Stdin data
    pub timeout_seconds: u32,
}

pub struct HermesResourceLimits {
    pub memory_limit_mb: u32,              // Default: 512
    pub cpu_limits: Option<String>,        // Docker CPU format: "0.5"
    pub disk_quota_mb: u32,                // Workspace size limit
}

pub struct SecretReference {
    pub id: String,                        // Secret ID
    pub env_var: String,                   // Environment variable to bind to
}

/// Response from Hermes when sub-agent spawned
pub struct HermesDelegateResponse {
    pub sub_agent_id: String,              // UUID of the sub-agent
    pub host: String,                      // IP:port to query
    pub container_id: String,              // Docker container ID
    pub status_url: String,                // URL to check status
}

/// Spawn a sub-agent via Hermes Gateway
pub async fn spawn_hermes_subagent(
    parent_agent: &Agent,
    request: HermesDelegateRequest,
    hermes_gateway: &HermesGateway,
    secret_service: &SecretService,
) -> Result<HermesDelegateResponse, DelegationError> {
    
    // Validate parent has delegation authority
    if !parent_agent.can_spawn_subagents {
        return Err(DelegationError::UnauthorizedDelegation(
            "Parent agent not authorized to spawn sub-agents".into()
        ));
    }
    
    // Resolve secret references (get actual values)
    let mut resolved_secrets = HashMap::new();
    for secret_ref in &request.secrets {
        let secret_value = secret_service.get_secret(&secret_ref.id)?;
        resolved_secrets.insert(secret_ref.env_var.clone(), secret_value);
    }
    
    // Build HTTP request to Hermes Gateway
    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/api/v1/agents/spawn", hermes_gateway.url))
        .header("X-Parent-Agent", parent_agent.id.clone())
        .header("X-Request-ID", uuid::Uuid::new_v4().to_string())
        .json(&json!({
            "parent_agent_id": parent_agent.id,
            "sub_agent_name": request.sub_agent_name,
            "image": request.image,
            "command": request.command,
            "environment": resolved_secrets,  // Pass secrets as env vars
            "resource_limits": {
                "memory_mb": request.resource_limits.memory_limit_mb,
                "cpu": request.resource_limits.cpu_limits,
                "disk_mb": request.resource_limits.disk_quota_mb,
            },
            "timeout_seconds": request.timeout_seconds,
        }))
        .send()
        .await
        .map_err(|e| DelegationError::HermesGatewayError(e.to_string()))?;
    
    if !response.status().is_success() {
        return Err(DelegationError::HermesGatewayError(
            format!("Hermes returned {}: {}", response.status(), response.text().await?)
        ));
    }
    
    let hermes_response: HermesDelegateResponse = response.json().await
        .map_err(|e| DelegationError::HermesGatewayError(e.to_string()))?;
    
    // Audit the delegation
    audit_log(AuditEvent::SubagentSpawned {
        parent_id: parent_agent.id.clone(),
        subagent_id: hermes_response.sub_agent_id.clone(),
        container_id: hermes_response.container_id.clone(),
        timestamp: SystemTime::now(),
    });
    
    Ok(hermes_response)
}
```

### 3.3 Hermes Docker Compose (Orchestrator)

```yaml
# hermes-docker-compose.yml
version: '3.9'

services:
  hermes-gateway:
    image: hermes-gateway:latest
    container_name: hermes-gateway
    networks:
      - zeroclaw-internal
      - hermes-mgmt
    ports:
      - "8080:8080"
    environment:
      - ZEROCLAW_HOST=zeroclaw-agent:3000
      - LOG_LEVEL=info
      - MAX_CONCURRENT_AGENTS=5
      - AGENT_TIMEOUT_SECONDS=300
      - WORKSPACE_ROOT=/hermes/agents
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro  # For spawning containers
      - zeroclaw-shared:/zeroclaw-data:ro              # Read shared workspace
      - hermes-workspace:/hermes/agents:rw
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
    sysctls:
      - net.ipv4.ip_forward=0

  hermes-worker-base:
    # Not run directly; used as base image for sub-agents
    image: hermes-worker:latest
    # This image is pulled and instantiated for each sub-agent spawn

volumes:
  zeroclaw-shared:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: /zeroclaw-data/workspace
  
  hermes-workspace:
    driver: local

networks:
  zeroclaw-internal:
    driver: bridge
    ipam:
      config:
        - subnet: 172.25.0.0/16
  
  hermes-mgmt:
    driver: bridge
    ipam:
      config:
        - subnet: 172.27.0.0/16
```

### 3.4 Sub-agent Filesystem Structure

```
/workspace/agents/
├── sub_agent_12345/              # Per sub-agent directory
│   ├── input/                    # Input data from parent
│   ├── output/                   # Results to return to parent
│   ├── tmp/                      # Temporary scratch space
│   ├── env                       # Environment configuration
│   └── .zeroclaw_meta            # Metadata JSON
│
└── shared/                       # Shared read-only resources (from ZeroClaw)
    ├── models/
    ├── configs/
    └── data/
```

### 3.5 Sub-agent Sandbox Dockerfile

```dockerfile
# Dockerfile for hermes-worker sub-agents
FROM debian:12-slim

# Minimal dependencies only
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Non-root user
RUN useradd -m -u 1000 -s /nologin agent

# Workspace directories
RUN mkdir -p /workspace/agents /workspace/shared && \
    chown -R agent:agent /workspace

# Copy application
COPY --chown=agent:agent ./app /app

WORKDIR /workspace

# Drop all capabilities
RUN setcap -r /usr/bin/curl 2>/dev/null || true

# Run as non-root
USER agent

# Entrypoint handles parent communication
ENTRYPOINT ["/app/entrypoint.sh"]
```

---

## 4. Subprocess Delegation (ByteRover MCP)

### 4.1 Architecture

ByteRover runs as a child process spawned via stdio. Communication is bidirectional over stdout/stdin with structured JSON messages.

```
┌─────────────────────────────────────────┐
│    ZeroClaw Rust Process                │
│                                         │
│  Subprocess: brv mcp --workspace=/ws    │
│  │                                      │
│  ├─ stdin  ──> [JSON commands]          │
│  │                                      │
│  └─ stdout <── [JSON responses]         │
│                                         │
│  ByteRover features:                    │
│  - File synchronization                 │
│  - Metadata search                      │
│  - Version control operations           │
│  - Cloud sync (via API)                │
│                                         │
└─────────────────────────────────────────┘
```

### 4.2 ByteRover MCP Integration

```rust
/// ByteRover MCP subprocess manager
pub struct ByteRoverMCP {
    pub process: tokio::process::Child,
    pub workspace_dir: PathBuf,
    pub session_id: String,
    pub json_rpc_id: Arc<AtomicU64>,
}

impl ByteRoverMCP {
    /// Spawn ByteRover MCP in workspace
    pub async fn new(workspace: &Path) -> Result<Self, MCPError> {
        // Validate workspace is within agent's bounds
        validate_workspace_path(workspace)?;
        
        // Build MCP command
        let mut cmd = tokio::process::Command::new("brv");
        cmd.arg("mcp")
            .arg("--workspace").arg(workspace)
            .arg("--format").arg("json-rpc")
            .arg("--timeout").arg("30");  // 30 second command timeout
        
        // Stdin/stdout for communication
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        
        // Restricted environment
        let mut env = HashMap::new();
        env.insert("ZEROCLAW_WORKSPACE".to_string(), workspace.to_string_lossy().to_string());
        env.insert("BR_FORMAT".to_string(), "json".to_string());
        
        for (key, value) in env {
            cmd.env(key, value);
        }
        
        let mut child = cmd.spawn()
            .map_err(|e| MCPError::SpawnFailed(e.to_string()))?;
        
        // Verify process started
        tokio::time::sleep(Duration::from_millis(100)).await;
        if child.try_wait().is_ok() {
            return Err(MCPError::ProcessTerminatedEarly);
        }
        
        Ok(Self {
            process: child,
            workspace_dir: workspace.to_path_buf(),
            session_id: uuid::Uuid::new_v4().to_string(),
            json_rpc_id: Arc::new(AtomicU64::new(1)),
        })
    }
    
    /// Execute ByteRover command
    pub async fn execute(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, MCPError> {
        // Generate request ID
        let id = self.json_rpc_id.fetch_add(1, Ordering::Relaxed);
        
        // Build JSON-RPC request
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        
        // Write to ByteRover stdin
        if let Some(ref mut stdin) = self.process.stdin {
            let mut writer = std::io::BufWriter::new(stdin);
            writeln!(writer, "{}", request.to_string())
                .map_err(|e| MCPError::WriteError(e.to_string()))?;
            writer.flush()
                .map_err(|e| MCPError::WriteError(e.to_string()))?;
        }
        
        // Read response from stdout with timeout
        let response = tokio::time::timeout(timeout, async {
            if let Some(ref mut stdout) = self.process.stdout {
                let mut reader = std::io::BufReader::new(stdout);
                let mut line = String::new();
                reader.read_line(&mut line)
                    .map_err(|e| MCPError::ReadError(e.to_string()))?;
                serde_json::from_str(&line)
                    .map_err(|e| MCPError::ParseError(e.to_string()))
            } else {
                Err(MCPError::NoStdout)
            }
        }).await
            .map_err(|_| MCPError::Timeout)?
            .map_err(|e| e)?;
        
        Ok(response)
    }
    
    /// Terminate ByteRover gracefully
    pub async fn shutdown(&mut self) -> Result<(), MCPError> {
        // Send graceful shutdown signal
        self.process.kill()
            .map_err(|e| MCPError::ShutdownFailed(e.to_string()))?;
        
        self.process.wait()
            .await
            .map_err(|e| MCPError::ShutdownFailed(e.to_string()))?;
        
        Ok(())
    }
}

pub enum MCPError {
    SpawnFailed(String),
    WriteError(String),
    ReadError(String),
    ParseError(String),
    Timeout,
    NoStdout,
    ShutdownFailed(String),
    ProcessTerminatedEarly,
}
```

### 4.3 ByteRover Workspace Constraints

```rust
/// Validate workspace path is within agent's bounds
fn validate_workspace_path(workspace: &Path) -> Result<(), MCPError> {
    // Get the canonical path
    let canonical = workspace.canonicalize()
        .map_err(|e| MCPError::InvalidWorkspace(format!("Cannot canonicalize: {}", e)))?;
    
    // Must be within /zeroclaw-data/workspace
    let root = Path::new("/zeroclaw-data/workspace");
    if !canonical.starts_with(root) {
        return Err(MCPError::InvalidWorkspace(
            format!("Workspace {} outside allowed root", canonical.display())
        ));
    }
    
    // Check for path traversal
    if canonical.to_string_lossy().contains("..") {
        return Err(MCPError::InvalidWorkspace("Path traversal detected".into()));
    }
    
    Ok(())
}

/// ByteRover MCP API operations (workspace-scoped)
pub enum ByteRoverOperation {
    // File operations
    ListFiles {
        path: String,
        recursive: bool,
    },
    ReadFile {
        path: String,
    },
    WriteFile {
        path: String,
        content: Vec<u8>,
    },
    DeleteFile {
        path: String,
    },
    
    // Metadata operations
    SearchFiles {
        query: String,
        limit: usize,
    },
    GetMetadata {
        path: String,
    },
    
    // Version control
    InitRepo { path: String },
    GetStatus { path: String },
    Sync { direction: SyncDirection },
    
    // Cloud operations
    CloudSync {
        path: String,
        direction: SyncDirection,
    },
}

pub enum SyncDirection {
    Upload,
    Download,
    Bidirectional,
}
```

---

## 5. Shared Resources & Cross-Agent Access

### 5.1 Shared Workspace Structure

```
/zeroclaw-data/workspace/
├── shared/                      # Accessible by all agents (with restrictions)
│   ├── models/                 # ML models (read-only for sub-agents)
│   ├── configs/                # Shared configuration files
│   ├── data/                   # Reference data (read-only)
│   └── .access_control         # Access control metadata
│
├── agents/                      # In-process delegates
│   ├── task_abc123/
│   │   ├── input/
│   │   ├── output/
│   │   └── tmp/
│   └── task_def456/
│
└── primary/                     # Primary agent workspace
    ├── data/
    ├── logs/
    ├── cache/
    └── tmp/
```

### 5.2 Access Control for Shared Resources

```rust
/// Shared resource access policy
pub struct SharedResourceAccess {
    pub resource_id: String,
    pub path: PathBuf,
    pub owner_agent_id: String,
    pub access_mode: AccessMode,
    pub allowed_agents: Vec<AgentPermission>,
}

pub enum AccessMode {
    ReadOnly,
    ReadWrite,
    Exclusive,  // Only owner can access
}

pub struct AgentPermission {
    pub agent_id: String,
    pub access: AccessMode,
    pub expires_at: Option<SystemTime>,
}

/// Check if agent can access shared resource
pub fn can_access_shared_resource(
    agent: &Agent,
    resource: &SharedResourceAccess,
) -> bool {
    // Owner always has access
    if agent.id == resource.owner_agent_id {
        return true;
    }
    
    // Check allowlist
    for perm in &resource.allowed_agents {
        if perm.agent_id == agent.id {
            // Check expiration
            if let Some(expires) = perm.expires_at {
                if SystemTime::now() > expires {
                    return false;  // Permission expired
                }
            }
            return true;
        }
    }
    
    false  // Not in allowlist
}

/// Read-only mount for sub-agents accessing shared resources
pub fn mount_shared_readonly(
    sub_agent_id: &str,
    container_config: &mut ContainerConfig,
) {
    // Mount shared/ as read-only
    container_config.add_volume_mount(
        VolumeMount {
            source: "/zeroclaw-data/workspace/shared".to_string(),
            target: "/workspace/shared".to_string(),
            read_only: true,
        }
    );
}
```

---

## 6. Audit & Logging

### 6.1 Inter-Agent Communication Audit Log

```rust
pub enum InterAgentEvent {
    DelegateSpawned {
        parent_id: String,
        delegate_id: String,
        delegation_type: String,  // in_process, container, subprocess
        timestamp: SystemTime,
    },
    DelegateExecuted {
        parent_id: String,
        delegate_id: String,
        command: String,
        exit_code: i32,
        resource_usage: ResourceUsage,
        timestamp: SystemTime,
    },
    DelegateSecretAccessed {
        parent_id: String,
        delegate_id: String,
        secret_id: String,
        timestamp: SystemTime,
    },
    DelegateSecretAccessDenied {
        parent_id: String,
        delegate_id: String,
        secret_id: String,
        reason: String,
        timestamp: SystemTime,
    },
    DelegateTerminated {
        parent_id: String,
        delegate_id: String,
        cleanup_status: String,
        timestamp: SystemTime,
    },
    SharedResourceAccessed {
        agent_id: String,
        resource_id: String,
        operation: String,  // read, write
        timestamp: SystemTime,
    },
}

/// Log all inter-agent events to audit trail
pub async fn audit_inter_agent_event(event: InterAgentEvent) {
    let json = serde_json::to_string(&event).unwrap_or_default();
    println!("[AUDIT] {}", json);
    
    // Also write to audit file
    // tokio::fs::append_to_file("/var/log/zeroclaw/audit.log", &format!("{}\n", json)).await;
}
```

---

## 7. Summary: Isolation Guarantees

| Guarantee | In-Process | Container | MCP |
|-----------|-----------|-----------|-----|
| **Filesystem isolation** | Scoped dir | Own root | Parent-bound |
| **Process isolation** | Same (Rust) | Separate OS | Child process |
| **Secret inheritance** | Explicit scope | Env vars | Config file |
| **Resource limits** | Rust-level | Docker limits | Process limits |
| **Re-delegation** | Depth=0 (blocked) | Allowed | N/A |
| **Execution time** | 30 seconds | 5 minutes | 30 seconds |
| **Memory limit** | 256 MB | 512 MB | Shared |

---

## 8. Implementation Checklist

- [ ] Implement `DelegateAgent` struct and spawn logic
- [ ] Add resource monitoring and enforcement
- [ ] Integrate Hermes HTTP gateway communication
- [ ] Implement ByteRover MCP subprocess spawning
- [ ] Add shared resource access control
- [ ] Audit logging for all inter-agent events
- [ ] Cleanup procedures for delegates
- [ ] Test cross-agent communication security
- [ ] Document delegation policies
- [ ] Deploy with comprehensive logging

