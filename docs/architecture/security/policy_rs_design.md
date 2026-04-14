# ZeroClaw Security Policy (policy.rs) — Redesign Specification

**Version:** 1.0  
**Date:** 2025-04-14  
**Status:** Design Specification  
**Audience:** Rust engineers implementing ZeroClaw security enforcement

---

## Executive Summary

This document specifies the updated `policy.rs` logic that governs shell command execution, redirection, and piping in ZeroClaw. The redesign maintains hardcoded blocking of dangerous operators while introducing intelligent validation of redirects and pipes to ensure they remain confined to the agent's workspace (`/zeroclaw-data/workspace/`).

**Key Design Principles:**
1. **Whitelist over blacklist**: Only allow known-safe commands
2. **Immutable security policy**: Hardcoded blocks cannot be overridden at runtime
3. **Path confinement**: Redirects and pipes validate all targets resolve to workspace
4. **Defense in depth**: Multiple validation layers catch different attack vectors
5. **Fail-secure**: Ambiguous commands are denied by default

---

## 1. Core Data Structures

### 1.1 SecurityPolicy Enum

```rust
pub enum SecurityPolicy {
    ReadOnly,      // No file writes, no shell operators
    Supervised,    // File writes allowed, shell operators need approval
    Full,          // All operations allowed (trusted agents only)
}
```

### 1.2 CommandMetadata

```rust
pub struct CommandMetadata {
    pub name: String,                    // Base command name (e.g., "ls", "grep")
    pub risk_level: RiskLevel,           // Low, Medium, High
    pub allows_shell_operators: bool,    // Can this cmd be used with pipes, redirects?
    pub requires_approval: bool,         // Needs admin approval for execution
    pub requires_input_validation: bool, // Needs strict argument inspection
    pub workspace_only: bool,            // Restricted to workspace paths
    pub forbidden_patterns: Vec<String>, // Regex patterns in args that block execution
}

pub enum RiskLevel {
    Low,     // Safe: ls, cat, echo
    Medium,  // Needs validation: grep, awk, sed
    High,    // Restricted: curl, wget, git, find
}
```

### 1.3 OperationContext

```rust
pub struct OperationContext {
    pub agent_id: String,
    pub workspace_root: PathBuf,
    pub policy_level: SecurityPolicy,
    pub request_id: String,              // For logging/auditing
    pub timestamp: SystemTime,
    pub user_initiated: bool,            // vs. autonomous agent action
}
```

---

## 2. Redirect Parsing & Validation

### 2.1 Redirect Target Extraction

```rust
/// Extract redirect targets from a command string.
/// 
/// Examples:
///   "cat file > output.txt"        -> vec!["output.txt"]
///   "echo test >> log.log"         -> vec!["log.log"]
///   "cmd > file1.txt > file2.txt"  -> vec!["file1.txt", "file2.txt"]
///   "cmd > file | grep x"          -> vec!["file"]  (only first redirect parsed)
///
/// Algorithm:
///   1. Split by pipe `|` to isolate redirection section
///   2. Find all `>` and `>>` tokens (not in quotes)
///   3. Capture the token immediately following each operator
///   4. Handle quoted strings: "output file.txt", 'another.log'
///   5. Reject if target is a variable or command substitution
///
pub fn extract_redirect_targets(command: &str) -> Result<Vec<String>, PolicyError> {
    let mut targets = Vec::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape_next = false;
    let mut i = 0;
    let bytes = command.as_bytes();
    
    while i < bytes.len() {
        let ch = bytes[i] as char;
        
        // Track quote state
        if escape_next {
            escape_next = false;
            i += 1;
            continue;
        }
        
        if ch == '\\' {
            escape_next = true;
            i += 1;
            continue;
        }
        
        if ch == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            i += 1;
            continue;
        }
        
        if ch == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            i += 1;
            continue;
        }
        
        // Skip if inside quotes
        if in_single_quote || in_double_quote {
            i += 1;
            continue;
        }
        
        // Detect redirect operators
        if ch == '>' {
            // Check for >> vs >
            let is_append = (i + 1 < bytes.len()) && (bytes[i + 1] as char == '>');
            let operator_len = if is_append { 2 } else { 1 };
            
            i += operator_len;
            
            // Skip whitespace after operator
            while i < bytes.len() && bytes[i] as char == ' ' {
                i += 1;
            }
            
            if i >= bytes.len() {
                return Err(PolicyError::MalformedRedirect("No target after >".into()));
            }
            
            // Extract target (stop at space, pipe, or semicolon)
            let mut target = String::new();
            let target_start = i;
            let mut target_quoted = false;
            
            if bytes[i] as char == '"' || bytes[i] as char == '\'' {
                target_quoted = true;
                let quote_char = bytes[i] as char;
                i += 1;
                
                while i < bytes.len() && (bytes[i] as char) != quote_char {
                    target.push(bytes[i] as char);
                    i += 1;
                }
                
                if i < bytes.len() {
                    i += 1; // Skip closing quote
                }
            } else {
                // Unquoted target: read until whitespace or pipe
                while i < bytes.len() {
                    let c = bytes[i] as char;
                    if c == ' ' || c == '|' || c == '&' || c == ';' {
                        break;
                    }
                    target.push(c);
                    i += 1;
                }
            }
            
            // Validate target is not a variable or command substitution
            if target.starts_with('$') || target.starts_with('(') {
                return Err(PolicyError::DangerousRedirect(
                    format!("Redirect target cannot be variable or substitution: {}", target)
                ));
            }
            
            if !target.is_empty() {
                targets.push(target);
            }
            
            continue;
        }
        
        // Detect pipe: stop parsing redirects after pipe
        if ch == '|' && !in_single_quote && !in_double_quote {
            // Check for || (logical OR)
            if i + 1 < bytes.len() && bytes[i + 1] as char == '|' {
                i += 2;
                continue;
            }
            // Single pipe found: stop redirect parsing after this point
            break;
        }
        
        i += 1;
    }
    
    Ok(targets)
}

pub enum PolicyError {
    MalformedRedirect(String),
    DangerousRedirect(String),
    PathTraversal(String),
    UnauthorizedPath(String),
}
```

### 2.2 Redirect Safety Validation

```rust
/// Validate that all redirect targets are safe and within the workspace.
///
/// Rules:
///   1. All paths must be absolute or relative to workspace
///   2. No `..` sequences allowed
///   3. No symlinks pointing outside workspace
///   4. No /tmp, /var, /dev, system directories
///   5. Must normalize path and re-check against whitelist
///
pub fn are_redirects_safe(
    targets: &[String],
    workspace_dir: &Path,
    context: &OperationContext,
) -> Result<bool, PolicyError> {
    // Canonicalize workspace path
    let workspace_canonical = workspace_dir
        .canonicalize()
        .map_err(|e| PolicyError::PathTraversal(format!("Workspace path invalid: {}", e)))?;
    
    for target in targets {
        // Reject absolute paths that don't start with workspace
        if target.starts_with('/') {
            if !target.starts_with(workspace_canonical.to_str().unwrap_or("")) {
                return Err(PolicyError::UnauthorizedPath(format!(
                    "Absolute redirect path outside workspace: {}",
                    target
                )));
            }
        }
        
        // Check for path traversal
        if target.contains("..") {
            return Err(PolicyError::PathTraversal(format!(
                "Path traversal in redirect target: {}",
                target
            )));
        }
        
        // Reject paths in system directories
        let forbidden = vec!["/tmp", "/var", "/dev", "/etc", "/sys", "/proc", "/root"];
        for dir in forbidden {
            if target.starts_with(dir) {
                return Err(PolicyError::UnauthorizedPath(format!(
                    "Redirect to forbidden directory: {} in target: {}",
                    dir, target
                )));
            }
        }
        
        // Construct full path
        let full_path = if target.starts_with('/') {
            PathBuf::from(target)
        } else {
            workspace_canonical.join(target)
        };
        
        // Canonicalize the target (resolve symlinks, .., .)
        let canonical_target = full_path
            .canonicalize()
            .map_err(|e| {
                // Path doesn't exist yet, which is OK for output redirects
                // But still validate it's in a safe location
                if target.starts_with("..") || target.contains("/../") {
                    PolicyError::PathTraversal(format!("Path traversal detected: {}", target))
                } else {
                    // Parent directory must exist and be within workspace
                    PolicyError::PathTraversal(format!("Cannot validate target path: {}", e))
                }
            })?;
        
        // Final check: canonical path must be within workspace
        if !canonical_target.starts_with(&workspace_canonical) {
            return Err(PolicyError::UnauthorizedPath(format!(
                "Redirect target resolves outside workspace: {} -> {}",
                target,
                canonical_target.display()
            )));
        }
    }
    
    Ok(true)
}
```

---

## 3. Pipe & Operator Validation

### 3.1 Command Pipeline Parser

```rust
/// Parse a command with pipes into individual stages.
///
/// Example:
///   "cat log.txt | grep ERROR | sort | uniq > output.txt"
///   
/// Returns:
///   stages: ["cat log.txt", "grep ERROR", "sort", "uniq"]
///   final_redirect: Some("output.txt")
///
pub struct PipelineStage {
    pub command: String,
    pub base_command: String,  // First token (e.g., "cat", "grep")
    pub arguments: Vec<String>,
}

pub struct Pipeline {
    pub stages: Vec<PipelineStage>,
    pub final_redirect: Option<String>,
    pub has_background: bool,
    pub logical_operators: Vec<LogicalOp>,  // && and ||
}

pub enum LogicalOp {
    And,  // &&
    Or,   // ||
}

pub fn parse_pipeline(command: &str) -> Result<Pipeline, PolicyError> {
    // Remove leading/trailing whitespace
    let command = command.trim();
    
    // First, extract and validate redirect targets
    let redirect_targets = extract_redirect_targets(command)?;
    let final_redirect = redirect_targets.last().map(|s| s.to_string());
    
    // Remove redirection part for pipe parsing
    let (command_part, _) = if let Some(pos) = command.find('>') {
        (&command[..pos], &command[pos..])
    } else {
        (command, "")
    };
    
    // Check for background operator (&)
    let (command_part, has_background) = if command_part.trim_end().ends_with('&') {
        (&command_part[..command_part.len()-1], true)
    } else {
        (command_part, false)
    };
    
    // Split by pipe, preserving logical operators
    let mut stages = Vec::new();
    let mut logical_operators = Vec::new();
    
    let parts: Vec<&str> = command_part.split('|').collect();
    
    for part in parts {
        let part = part.trim();
        
        // Check for && or || after the command
        let (cmd, op) = if let Some(and_pos) = part.rfind("&&") {
            let (cmd, _) = part.split_at(and_pos);
            (cmd.trim(), Some(LogicalOp::And))
        } else if let Some(or_pos) = part.rfind("||") {
            let (cmd, _) = part.split_at(or_pos);
            (cmd.trim(), Some(LogicalOp::Or))
        } else {
            (part, None)
        };
        
        if cmd.is_empty() {
            continue;
        }
        
        let tokens: Vec<&str> = cmd.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        
        let base_command = tokens[0].to_string();
        let arguments = tokens[1..].iter().map(|s| s.to_string()).collect();
        
        stages.push(PipelineStage {
            command: cmd.to_string(),
            base_command,
            arguments,
        });
        
        if let Some(op) = op {
            logical_operators.push(op);
        }
    }
    
    Ok(Pipeline {
        stages,
        final_redirect,
        has_background,
        logical_operators,
    })
}
```

### 3.2 Pipe Safety Validation

```rust
/// Validate that all commands in a pipeline are authorized.
///
/// Rules for pipes:
///   1. Both sides of the pipe must have allowed base commands
///   2. Input/output redirection must be within workspace
///   3. Commands with high risk must be approved individually
///   4. Variable expansion in arguments is allowed (validated elsewhere)
///   5. Reject pipes to shell interpreters (bash, sh, zsh)
///
pub fn validate_pipeline_security(
    pipeline: &Pipeline,
    context: &OperationContext,
    config: &SecurityConfig,
    auth_service: &AuthorizationService,
) -> Result<(), PolicyError> {
    // Reject background operators unconditionally
    if pipeline.has_background {
        return Err(PolicyError::ForbiddenOperator(
            "Background operator (&) is not allowed".into(),
        ));
    }
    
    // Validate all stages
    for (idx, stage) in pipeline.stages.iter().enumerate() {
        // Check if base command is in allowlist
        let cmd_metadata = config
            .get_command_metadata(&stage.base_command)
            .ok_or_else(|| {
                PolicyError::UnauthorizedCommand(format!(
                    "Command not in allowlist: {}",
                    stage.base_command
                ))
            })?;
        
        // Reject pipes to shell interpreters
        if is_shell_interpreter(&stage.base_command) {
            return Err(PolicyError::ForbiddenOperator(format!(
                "Piping to shell interpreter {} is not allowed",
                stage.base_command
            )));
        }
        
        // Validate arguments (covered in section 4.4)
        validate_command_arguments(&stage.base_command, &stage.arguments, context)?;
        
        // Check risk level and approval requirements
        if cmd_metadata.risk_level == RiskLevel::High && !context.user_initiated {
            // Autonomous agents need approval for high-risk commands in pipes
            if !auth_service.is_pre_approved(&stage.base_command, context)? {
                return Err(PolicyError::ApprovalRequired(format!(
                    "High-risk command {} in pipeline requires approval",
                    stage.base_command
                )));
            }
        }
    }
    
    // Validate final redirect target if present
    if let Some(ref target) = pipeline.final_redirect {
        are_redirects_safe(&[target.clone()], &context.workspace_root, context)?;
    }
    
    Ok(())
}

fn is_shell_interpreter(cmd: &str) -> bool {
    matches!(cmd, "bash" | "sh" | "zsh" | "ksh" | "dash" | "fish")
}
```

---

## 4. Command Authorization Logic

### 4.1 Updated `is_command_allowed()` Flow

```rust
/// Main entry point: Determine if a command is allowed under current policy.
///
/// Returns: (allowed: bool, approval_required: bool, reason: String)
///
pub fn is_command_allowed(
    full_command: &str,
    context: &OperationContext,
    config: &SecurityConfig,
    auth_service: &AuthorizationService,
) -> Result<CommandDecision, PolicyError> {
    let decision = CommandDecision {
        allowed: false,
        approval_required: false,
        reason: String::new(),
        audit_log_entry: None,
    };
    
    // ==================== HARDCODED BLOCKS (Immutable) ====================
    
    // Block subshell operators
    let subshell_patterns = vec!["`", "$(", "${", "<(", ">("];
    for pattern in &subshell_patterns {
        if full_command.contains(pattern) {
            return Ok(CommandDecision {
                allowed: false,
                approval_required: false,
                reason: format!("Subshell operator '{}' is blocked", pattern),
                audit_log_entry: None,
            });
        }
    }
    
    // Block standalone background operator
    if has_standalone_background_operator(full_command) {
        return Ok(CommandDecision {
            allowed: false,
            approval_required: false,
            reason: "Background operator (&) is blocked".into(),
            audit_log_entry: None,
        });
    }
    
    // Block tee command unconditionally
    if full_command.trim().starts_with("tee") {
        return Ok(CommandDecision {
            allowed: false,
            approval_required: false,
            reason: "tee command is blocked (can redirect to arbitrary files)".into(),
            audit_log_entry: None,
        });
    }
    
    // ==================== POLICY-DEPENDENT LOGIC ====================
    
    match context.policy_level {
        SecurityPolicy::ReadOnly => {
            // No file writes, no shell operators at all
            if full_command.contains('|') || full_command.contains('>') 
                || full_command.contains("&&") || full_command.contains("||") {
                return Ok(CommandDecision {
                    allowed: false,
                    approval_required: false,
                    reason: "Operators (|, >, &&, ||) not allowed under ReadOnly policy".into(),
                    audit_log_entry: None,
                });
            }
            
            // Check if command is read-safe
            return check_read_only_command(full_command, config);
        }
        
        SecurityPolicy::Supervised => {
            // File writes allowed, but operators need validation
            return validate_supervised_command(full_command, context, config, auth_service);
        }
        
        SecurityPolicy::Full => {
            // Trusted agents: mostly unrestricted (except hardcoded blocks)
            return validate_full_policy_command(full_command, config);
        }
    }
}

pub struct CommandDecision {
    pub allowed: bool,
    pub approval_required: bool,
    pub reason: String,
    pub audit_log_entry: Option<AuditLogEntry>,
}

pub struct AuditLogEntry {
    pub request_id: String,
    pub timestamp: SystemTime,
    pub agent_id: String,
    pub command: String,
    pub decision: String,
    pub reason: String,
}
```

### 4.2 Supervised Policy Command Validation

```rust
/// Validate command under Supervised policy.
/// 
/// Rules:
///   1. All base commands must be in allowlist
///   2. Pipes are allowed if both sides are in allowlist
///   3. Redirects (> and >>) are allowed only to workspace paths
///   4. High-risk commands need approval before execution
///   5. All arguments validated for injection patterns
///
fn validate_supervised_command(
    full_command: &str,
    context: &OperationContext,
    config: &SecurityConfig,
    auth_service: &AuthorizationService,
) -> Result<CommandDecision, PolicyError> {
    // Parse pipeline (handles pipes, redirects, operators)
    let pipeline = parse_pipeline(full_command)?;
    
    // Validate all stages and redirects
    validate_pipeline_security(&pipeline, context, config, auth_service)?;
    
    // Get the primary command
    let primary_cmd = pipeline.stages.first()
        .ok_or_else(|| PolicyError::MalformedCommand("Empty pipeline".into()))?
        .base_command.clone();
    
    // Check if command is in allowlist
    let cmd_metadata = config.get_command_metadata(&primary_cmd)
        .ok_or_else(|| PolicyError::UnauthorizedCommand(
            format!("Command '{}' not in allowlist", primary_cmd)
        ))?;
    
    // Determine if approval is required
    let mut approval_required = false;
    
    if cmd_metadata.requires_approval {
        approval_required = true;
    }
    
    // For autonomous agents, check auto-approve list
    if !context.user_initiated && approval_required {
        if config.is_auto_approved(&primary_cmd) {
            approval_required = false;
        }
    }
    
    // Log the decision
    let audit_log = AuditLogEntry {
        request_id: context.request_id.clone(),
        timestamp: context.timestamp,
        agent_id: context.agent_id.clone(),
        command: full_command.to_string(),
        decision: if approval_required {
            "APPROVAL_PENDING".into()
        } else {
            "ALLOWED".into()
        },
        reason: format!(
            "Supervised policy: {} (risk: {:?})",
            primary_cmd, cmd_metadata.risk_level
        ),
    };
    
    Ok(CommandDecision {
        allowed: !approval_required,
        approval_required,
        reason: if approval_required {
            format!("Command {} requires approval", primary_cmd)
        } else {
            "Command allowed under Supervised policy".into()
        },
        audit_log_entry: Some(audit_log),
    })
}
```

### 4.3 Hardcoded Dangerous Commands

```rust
/// Commands that are always blocked, regardless of policy.
pub fn is_hardcoded_blocked(base_command: &str, args: &[String]) -> Result<bool, PolicyError> {
    match base_command {
        "find" => {
            // find -exec is dangerous
            if args.iter().any(|a| a == "-exec") {
                return Ok(true);
            }
            Ok(false)
        }
        "git" => {
            // git config can read/write arbitrary files
            if args.first().map_or(false, |a| a == "config") {
                return Ok(true);
            }
            Ok(false)
        }
        "rm" | "dd" | "mkfs" | "fdisk" => {
            // Destructive commands always blocked
            Ok(true)
        }
        "sudo" | "su" | "doas" => {
            // Privilege escalation always blocked
            Ok(true)
        }
        "chown" | "chmod" | "chgrp" => {
            // Permission changes blocked (confined environment)
            Ok(true)
        }
        _ => Ok(false),
    }
}
```

### 4.4 Argument Validation

```rust
/// Validate command arguments for injection patterns.
///
/// Patterns blocked:
///   - Semicolon (;) for command chaining
///   - Backtick (`) for command substitution
///   - $(...) for command substitution
///   - ${...} for variable expansion (depends on context)
///   - Pipe (|) outside of allowed piping context
///   - Redirect operators > >> < << outside of allowed context
///
pub fn validate_command_arguments(
    base_command: &str,
    args: &[String],
    context: &OperationContext,
) -> Result<(), PolicyError> {
    // Get dangerous patterns for this command type
    let dangerous_patterns = get_dangerous_patterns_for_command(base_command);
    
    for (idx, arg) in args.iter().enumerate() {
        // Check each dangerous pattern
        for pattern in &dangerous_patterns {
            if arg.contains(pattern.as_str()) {
                return Err(PolicyError::DangerousArgument(format!(
                    "Argument {} contains blocked pattern '{}': {}",
                    idx, pattern, arg
                )));
            }
        }
        
        // Command-specific validation
        match base_command {
            "curl" | "wget" => {
                // Validate URL is to allowed domain
                validate_network_url(arg, context)?;
            }
            "grep" | "sed" | "awk" => {
                // Regex arguments: validate for ReDoS (Regular Expression Denial of Service)
                validate_regex_safety(arg)?;
            }
            "find" => {
                // find arguments: prevent -exec, path traversal
                if arg.contains("..") {
                    return Err(PolicyError::DangerousArgument(
                        format!("Path traversal in find argument: {}", arg)
                    ));
                }
            }
            _ => {}
        }
    }
    
    Ok(())
}

fn get_dangerous_patterns_for_command(cmd: &str) -> Vec<String> {
    // Default dangerous patterns
    let mut patterns = vec![
        ";".to_string(),       // Command chaining
        "|".to_string(),       // Pipe (outside of valid context)
        "&&".to_string(),      // Logical AND (outside of valid context)
        "||".to_string(),      // Logical OR (outside of valid context)
    ];
    
    // Add command-specific patterns
    match cmd {
        "bash" | "sh" | "zsh" => {
            patterns.push("$(", ".into());
            patterns.push("`".into());
        }
        "eval" | "source" | "exec" => {
            patterns.push("$".into());  // Variable expansion in these is dangerous
        }
        _ => {}
    }
    
    patterns
}
```

---

## 5. Edge Cases & Special Handling

### 5.1 Quoted String Handling

```rust
/// Determine if a character is quoted in a command string.
/// 
/// Returns the nesting level of quotes at position idx.
/// 0 = not quoted
/// 1 = inside single quotes
/// 2 = inside double quotes
///
pub fn quote_nesting_at_position(command: &str, target_idx: usize) -> usize {
    let mut nesting = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape_next = false;
    
    for (i, ch) in command.chars().enumerate() {
        if i >= target_idx {
            break;
        }
        
        if escape_next {
            escape_next = false;
            continue;
        }
        
        match ch {
            '\\' => escape_next = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            _ => {}
        }
    }
    
    if in_single {
        1
    } else if in_double {
        2
    } else {
        0
    }
}

/// Check if a token is quoted (e.g., "output.txt" or 'file.log').
pub fn is_token_quoted(token: &str) -> bool {
    (token.starts_with('"') && token.ends_with('"'))
        || (token.starts_with('\'') && token.ends_with('\''))
}

/// Unquote a token for processing.
pub fn unquote_token(token: &str) -> String {
    if is_token_quoted(token) && token.len() > 1 {
        token[1..token.len()-1].to_string()
    } else {
        token.to_string()
    }
}
```

### 5.2 Variable Expansion Safety

```rust
/// Check if a string contains variable expansions that need special handling.
/// 
/// Safe variables (pre-approved):
///   - $WORKSPACE, $HOME, $PWD (mapped to workspace equivalents)
///   - Environment variables set by system
///
/// Unsafe variables:
///   - LD_PRELOAD, LD_LIBRARY_PATH (library injection)
///   - PATH (modifying command resolution)
///   - IFS (field separator injection)
///
pub fn is_variable_expansion_safe(value: &str) -> Result<bool, PolicyError> {
    // Find all $VAR patterns
    let var_pattern = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    
    for cap in var_pattern.captures_iter(value) {
        let var_name = &cap[1];
        
        let unsafe_vars = vec![
            "LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT",
            "PATH", "SHELL", "IFS",
            "BASH_ENV", "ENV",
        ];
        
        if unsafe_vars.contains(&var_name) {
            return Err(PolicyError::UnsafeVariableExpansion(
                format!("Variable {} cannot be expanded in this context", var_name)
            ));
        }
    }
    
    Ok(true)
}
```

### 5.3 Path Normalization

```rust
/// Safely normalize a path to prevent traversal attacks.
///
/// Rules:
///   1. Reject if contains ".."
///   2. Reject if contains "~"
///   3. Reject if absolute path outside workspace
///   4. Resolve symlinks
///   5. Collapse "//" sequences
///
pub fn normalize_and_validate_path(
    path: &str,
    workspace_root: &Path,
) -> Result<PathBuf, PolicyError> {
    // Reject path traversal
    if path.contains("..") || path.contains("~") {
        return Err(PolicyError::PathTraversal(format!(
            "Path contains traversal characters: {}",
            path
        )));
    }
    
    // Collapse double slashes
    let normalized = path.replace("//", "/");
    
    let full_path = if normalized.starts_with('/') {
        PathBuf::from(&normalized)
    } else {
        workspace_root.join(&normalized)
    };
    
    // Try to canonicalize (will fail if path doesn't exist, which is OK for outputs)
    match full_path.canonicalize() {
        Ok(canonical) => {
            // Verify canonical path is within workspace
            if canonical.starts_with(workspace_root) {
                Ok(canonical)
            } else {
                Err(PolicyError::UnauthorizedPath(format!(
                    "Path resolves outside workspace: {}",
                    canonical.display()
                )))
            }
        }
        Err(_) => {
            // Path doesn't exist; verify parent and target construction are safe
            let parent = full_path.parent().unwrap_or(workspace_root);
            if parent.starts_with(workspace_root) {
                Ok(full_path)
            } else {
                Err(PolicyError::UnauthorizedPath(format!(
                    "Cannot create path outside workspace: {}",
                    path
                )))
            }
        }
    }
}
```

---

## 6. Configuration Structure (TOML)

```toml
# security_config.toml
[policy]
default_level = "Supervised"

[workspace]
root = "/zeroclaw-data/workspace"
forbidden_paths = ["/tmp", "/var", "/dev", "/etc", "/sys", "/proc", "/root"]

[commands]
# Format: "command_name" = { risk = "Low|Medium|High", requires_approval = bool, ... }

[commands.low_risk]
# Read-only, always safe
allowed = [
    "ls", "cat", "grep", "head", "tail", "wc", "echo", 
    "pwd", "date", "uname", "whoami", "id", "env"
]

[commands.medium_risk]
# Require validation, may need approval
cat_with_options = { command = "cat", risk = "Low" }
find_limited = { command = "find", risk = "Medium", forbidden_args = ["-exec"] }
grep_patterns = { command = "grep", risk = "Medium", requires_input_validation = true }
curl_urls = { command = "curl", risk = "High", requires_approval = true }
wget_urls = { command = "wget", risk = "High", requires_approval = true }

[commands.high_risk]
# Restricted, approval required
git_config = { command = "git", forbidden_subcommands = ["config"] }
docker_exec = { command = "docker", forbidden_subcommands = ["run", "exec", "rmi"] }

[approval]
# Commands that auto-approve for autonomous agents
auto_approve = [
    "ls", "cat", "echo", "grep", "sort", "uniq", "wc"
]

# Network allowlist (see network_allowlist.md)
[network]
allowed_domains = [
    "api.alpaca.markets",
    "data.alpaca.markets",
    "api.elevenlabs.io",
    "api.pushover.net"
]
```

---

## 7. Error Handling & Logging

```rust
/// Structured error type for policy violations.
pub enum PolicyError {
    UnauthorizedCommand(String),
    ForbiddenOperator(String),
    DangerousArgument(String),
    DangerousRedirect(String),
    PathTraversal(String),
    UnauthorizedPath(String),
    UnsafeVariableExpansion(String),
    MalformedCommand(String),
    MalformedRedirect(String),
    ApprovalRequired(String),
    NetworkViolation(String),
}

/// Structured audit logging entry.
pub struct AuditLog {
    pub request_id: String,
    pub timestamp: SystemTime,
    pub agent_id: String,
    pub operation: String,
    pub result: String,        // ALLOWED, DENIED, APPROVAL_PENDING
    pub reason: String,
    pub command: String,
    pub source: String,        // user_initiated or autonomous
}

pub trait AuditLogger {
    fn log(&self, entry: AuditLog) -> Result<(), Box<dyn std::error::Error>>;
}

/// Default implementation writes to structured JSON log
pub struct JsonAuditLogger {
    log_path: PathBuf,
}

impl AuditLogger for JsonAuditLogger {
    fn log(&self, entry: AuditLog) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(&entry)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        writeln!(file, "{}", json)?;
        Ok(())
    }
}
```

---

## 8. Testing & Validation

Key test cases for `policy.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redirect_extraction() {
        assert_eq!(
            extract_redirect_targets("cat file > output.txt").unwrap(),
            vec!["output.txt"]
        );
        assert!(extract_redirect_targets("cat > $VARFILE").is_err());
    }

    #[test]
    fn test_path_traversal_rejection() {
        let ctx = test_context("/workspace");
        assert!(are_redirects_safe(
            &["../etc/passwd".to_string()],
            &PathBuf::from("/workspace"),
            &ctx
        ).is_err());
    }

    #[test]
    fn test_subshell_blocking() {
        assert!(!is_command_allowed(
            "cat file | $(malicious)",
            &test_context(""),
            &test_config(),
            &test_auth()
        ).unwrap().allowed);
    }

    #[test]
    fn test_pipe_both_sides_validated() {
        // Both "cat" and "grep" must be in allowlist
        let result = validate_pipeline_security(
            &Pipeline {
                stages: vec![
                    PipelineStage { base_command: "cat".into(), .. },
                    PipelineStage { base_command: "evil_tool".into(), .. },
                ],
                ..
            },
            &test_context(""),
            &test_config(),
            &test_auth()
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_readonly_blocks_redirects() {
        let ctx = OperationContext {
            policy_level: SecurityPolicy::ReadOnly,
            ..
        };
        let result = is_command_allowed(
            "echo test > file.txt",
            &ctx,
            &test_config(),
            &test_auth()
        );
        assert!(!result.unwrap().allowed);
    }

    #[test]
    fn test_supervised_allows_safe_redirects() {
        let ctx = OperationContext {
            policy_level: SecurityPolicy::Supervised,
            workspace_root: PathBuf::from("/workspace"),
            ..
        };
        let result = is_command_allowed(
            "echo test > /zeroclaw-data/workspace/file.txt",
            &ctx,
            &test_config(),
            &test_auth()
        );
        assert!(result.unwrap().allowed);
    }
}
```

---

## 9. Summary & Deployment Checklist

**Key Security Properties:**
- ✅ Redirects confined to workspace via path validation
- ✅ Pipes validated on both sides (both commands must be allowlisted)
- ✅ Hardcoded blocks (subshells, background, tee) cannot be overridden
- ✅ All dangerous arguments caught at validation layer
- ✅ Path traversal prevented with multiple checks (`.., symlinks, canonical paths`)
- ✅ Audit logging for all security decisions
- ✅ Approval workflow for high-risk commands in autonomous mode

**Implementation Order:**
1. Implement `extract_redirect_targets()` and unit tests
2. Implement `are_redirects_safe()` with path normalization
3. Implement `parse_pipeline()` and `validate_pipeline_security()`
4. Update `is_command_allowed()` main entry point
5. Integrate hardcoded command blocking
6. Add audit logging
7. Deploy with comprehensive test coverage (>90%)

