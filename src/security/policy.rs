use parking_lot::Mutex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// How much autonomy the agent has
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AutonomyLevel {
    /// Read-only: can observe but not act
    ReadOnly,
    /// Supervised: acts but requires approval for risky operations
    #[default]
    Supervised,
    /// Full: autonomous execution within policy bounds
    Full,
}

/// Risk score for shell command execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRiskLevel {
    Low,
    Medium,
    High,
}

/// Classifies whether a tool operation is read-only or side-effecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOperation {
    Read,
    Act,
}

/// Sliding-window action tracker for rate limiting.
#[derive(Debug)]
pub struct ActionTracker {
    /// Timestamps of recent actions (kept within the last hour).
    actions: Mutex<Vec<Instant>>,
}

impl ActionTracker {
    pub fn new() -> Self {
        Self {
            actions: Mutex::new(Vec::new()),
        }
    }

    /// Record an action and return the current count within the window.
    pub fn record(&self) -> usize {
        let mut actions = self.actions.lock();
        let cutoff = Instant::now()
            .checked_sub(std::time::Duration::from_secs(3600))
            .unwrap_or_else(Instant::now);
        actions.retain(|t| *t > cutoff);
        actions.push(Instant::now());
        actions.len()
    }

    /// Count of actions in the current window without recording.
    pub fn count(&self) -> usize {
        let mut actions = self.actions.lock();
        let cutoff = Instant::now()
            .checked_sub(std::time::Duration::from_secs(3600))
            .unwrap_or_else(Instant::now);
        actions.retain(|t| *t > cutoff);
        actions.len()
    }
}

impl Clone for ActionTracker {
    fn clone(&self) -> Self {
        let actions = self.actions.lock();
        Self {
            actions: Mutex::new(actions.clone()),
        }
    }
}

/// Security policy enforced on all tool executions
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub autonomy: AutonomyLevel,
    pub workspace_dir: PathBuf,
    pub workspace_only: bool,
    pub allowed_commands: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub max_actions_per_hour: u32,
    pub max_cost_per_day_cents: u32,
    pub require_approval_for_medium_risk: bool,
    pub block_high_risk_commands: bool,
    pub tracker: ActionTracker,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            autonomy: AutonomyLevel::Supervised,
            workspace_dir: PathBuf::from("."),
            workspace_only: true,
            allowed_commands: vec![
                "git".into(),
                "npm".into(),
                "cargo".into(),
                "ls".into(),
                "cat".into(),
                "grep".into(),
                "find".into(),
                "echo".into(),
                "pwd".into(),
                "wc".into(),
                "head".into(),
                "tail".into(),
                "date".into(),
            ],
            forbidden_paths: vec![
                // System directories (blocked even when workspace_only=false)
                "/etc".into(),
                "/root".into(),
                "/home".into(),
                "/usr".into(),
                "/bin".into(),
                "/sbin".into(),
                "/lib".into(),
                "/opt".into(),
                "/boot".into(),
                "/dev".into(),
                "/proc".into(),
                "/sys".into(),
                "/var".into(),
                "/tmp".into(),
                // Sensitive dotfiles
                "~/.ssh".into(),
                "~/.gnupg".into(),
                "~/.aws".into(),
                "~/.config".into(),
            ],
            max_actions_per_hour: 20,
            max_cost_per_day_cents: 500,
            require_approval_for_medium_risk: true,
            block_high_risk_commands: true,
            tracker: ActionTracker::new(),
        }
    }
}

// ── Shell Command Parsing Utilities ───────────────────────────────────────
// These helpers implement a minimal quote-aware shell lexer. They exist
// because security validation must reason about the *structure* of a
// command (separators, operators, quoting) rather than treating it as a
// flat string — otherwise an attacker could hide dangerous sub-commands
// inside quoted arguments or chained operators.
/// Skip leading environment variable assignments (e.g. `FOO=bar cmd args`).
/// Returns the remainder starting at the first non-assignment word.
fn skip_env_assignments(s: &str) -> &str {
    let mut rest = s;
    loop {
        let Some(word) = rest.split_whitespace().next() else {
            return rest;
        };
        // Environment assignment: contains '=' and starts with a letter or underscore
        if word.contains('=')
            && word
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            // Advance past this word
            rest = rest[word.len()..].trim_start();
        } else {
            return rest;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteState {
    None,
    Single,
    Double,
}

/// Split a shell command into sub-commands by unquoted separators.
///
/// Separators:
/// - `;` and newline
/// - `|`
/// - `&&`, `||`
///
/// Characters inside single or double quotes are treated as literals, so
/// `sqlite3 db "SELECT 1; SELECT 2;"` remains a single segment.
fn split_unquoted_segments(command: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut quote = QuoteState::None;
    let mut escaped = false;
    let mut chars = command.chars().peekable();

    let push_segment = |segments: &mut Vec<String>, current: &mut String| {
        let trimmed = current.trim();
        if !trimmed.is_empty() {
            segments.push(trimmed.to_string());
        }
        current.clear();
    };

    while let Some(ch) = chars.next() {
        match quote {
            QuoteState::Single => {
                if ch == '\'' {
                    quote = QuoteState::None;
                }
                current.push(ch);
            }
            QuoteState::Double => {
                if escaped {
                    escaped = false;
                    current.push(ch);
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    current.push(ch);
                    continue;
                }
                if ch == '"' {
                    quote = QuoteState::None;
                }
                current.push(ch);
            }
            QuoteState::None => {
                if escaped {
                    escaped = false;
                    current.push(ch);
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    current.push(ch);
                    continue;
                }

                match ch {
                    '\'' => {
                        quote = QuoteState::Single;
                        current.push(ch);
                    }
                    '"' => {
                        quote = QuoteState::Double;
                        current.push(ch);
                    }
                    ';' | '\n' => push_segment(&mut segments, &mut current),
                    '|' => {
                        if chars.next_if_eq(&'|').is_some() {
                            // Consume full `||`; both characters are separators.
                        }
                        push_segment(&mut segments, &mut current);
                    }
                    '&' => {
                        if chars.next_if_eq(&'&').is_some() {
                            // `&&` is a separator; single `&` is handled separately.
                            push_segment(&mut segments, &mut current);
                        } else {
                            current.push(ch);
                        }
                    }
                    _ => current.push(ch),
                }
            }
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }

    segments
}

/// Detect a single unquoted `&` operator (background/chain). `&&` is allowed.
///
/// We treat any standalone `&` as unsafe in policy validation because it can
/// chain hidden sub-commands and escape foreground timeout expectations.
fn contains_unquoted_single_ampersand(command: &str) -> bool {
    let mut quote = QuoteState::None;
    let mut escaped = false;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        match quote {
            QuoteState::Single => {
                if ch == '\'' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::Double => {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '"' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::None => {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                match ch {
                    '\'' => quote = QuoteState::Single,
                    '"' => quote = QuoteState::Double,
                    '&' => {
                        if chars.next_if_eq(&'&').is_none() {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    false
}

/// Detect an unquoted character in a shell command.
fn contains_unquoted_char(command: &str, target: char) -> bool {
    let mut quote = QuoteState::None;
    let mut escaped = false;

    for ch in command.chars() {
        match quote {
            QuoteState::Single => {
                if ch == '\'' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::Double => {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '"' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::None => {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                match ch {
                    '\'' => quote = QuoteState::Single,
                    '"' => quote = QuoteState::Double,
                    _ if ch == target => return true,
                    _ => {}
                }
            }
        }
    }

    false
}

// ── Redirect Target Extraction & Validation ──────────────────────────────
// When a command contains shell redirect operators (`>`, `>>`, `<`), we
// extract the file paths they reference and verify every target lives
// inside the configured workspace directory. This allows safe patterns
// like `ls | grep foo > workspace/results.txt` while still blocking
// `echo secrets > /etc/crontab`.

/// Extract file-path targets from redirect operators (`>`, `>>`, `<`) in
/// a single shell segment (no pipes / `&&` / `||` — those are already
/// split by `split_unquoted_segments`).
///
/// Returns a list of path strings that the command would read from or
/// write to via shell I/O redirection.
fn extract_redirect_targets(segment: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut quote = QuoteState::None;
    let mut escaped = false;
    let mut i = 0;
    let bytes = segment.as_bytes();
    let len = bytes.len();

    while i < len {
        let ch = bytes[i] as char;

        // Handle quoting state
        match quote {
            QuoteState::Single => {
                if ch == '\'' {
                    quote = QuoteState::None;
                }
                i += 1;
                continue;
            }
            QuoteState::Double => {
                if escaped {
                    escaped = false;
                    i += 1;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    i += 1;
                    continue;
                }
                if ch == '"' {
                    quote = QuoteState::None;
                }
                i += 1;
                continue;
            }
            QuoteState::None => {}
        }

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if ch == '\'' {
            quote = QuoteState::Single;
            i += 1;
            continue;
        }
        if ch == '"' {
            quote = QuoteState::Double;
            i += 1;
            continue;
        }

        // Detect redirect operators outside quotes
        if ch == '>' || ch == '<' {
            // Skip `>>` as a unit
            if ch == '>' && i + 1 < len && bytes[i + 1] == b'>' {
                i += 2;
            } else {
                i += 1;
            }
            // Skip optional whitespace after the operator
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            // Collect the target path token (respecting quotes)
            let mut target = String::new();
            let mut tq = QuoteState::None;
            let mut tesc = false;
            while i < len {
                let tc = bytes[i] as char;
                match tq {
                    QuoteState::Single => {
                        if tc == '\'' {
                            tq = QuoteState::None;
                        } else {
                            target.push(tc);
                        }
                        i += 1;
                    }
                    QuoteState::Double => {
                        if tesc {
                            tesc = false;
                            target.push(tc);
                            i += 1;
                            continue;
                        }
                        if tc == '\\' {
                            tesc = true;
                            i += 1;
                            continue;
                        }
                        if tc == '"' {
                            tq = QuoteState::None;
                        } else {
                            target.push(tc);
                        }
                        i += 1;
                    }
                    QuoteState::None => {
                        if tesc {
                            tesc = false;
                            target.push(tc);
                            i += 1;
                            continue;
                        }
                        if tc == '\\' {
                            tesc = true;
                            i += 1;
                            continue;
                        }
                        if tc == '\'' {
                            tq = QuoteState::Single;
                            i += 1;
                            continue;
                        }
                        if tc == '"' {
                            tq = QuoteState::Double;
                            i += 1;
                            continue;
                        }
                        // Unquoted whitespace or operator ends the token
                        if tc == ' ' || tc == '\t' || tc == '>' || tc == '<'
                            || tc == '|' || tc == '&' || tc == ';' || tc == '\n'
                        {
                            break;
                        }
                        target.push(tc);
                        i += 1;
                    }
                }
            }
            let trimmed = target.trim().to_string();
            if !trimmed.is_empty() {
                targets.push(trimmed);
            }
            continue;
        }

        i += 1;
    }

    targets
}

/// Check whether **all** redirect targets in a command are safe, i.e.
/// they resolve to paths inside `workspace_dir` and contain no path
/// traversal components.
///
/// A target is safe when:
/// 1. It contains no `..` path components (prevents traversal).
/// 2. If it is a relative path, joining it with `workspace_dir` stays
///    inside the workspace.
/// 3. If it is an absolute path, it must start with `workspace_dir`.
///
/// Returns `true` if there are no redirect targets or all of them are
/// safe. Returns `false` if any target escapes the workspace.
fn are_redirects_safe(targets: &[String], workspace_dir: &Path) -> bool {
    for target in targets {
        let p = Path::new(target);

        // Block path traversal (`..` components)
        if p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return false;
        }

        // Block null bytes
        if target.contains('\0') {
            return false;
        }

        // Resolve the effective path
        let resolved = if p.is_absolute() {
            p.to_path_buf()
        } else {
            workspace_dir.join(p)
        };

        // Normalise away any `.` components without touching the filesystem
        // (canonicalize() would fail on non-existent paths).
        let mut normalised = PathBuf::new();
        for comp in resolved.components() {
            match comp {
                std::path::Component::ParentDir => {
                    // Already blocked above, but defend in depth
                    return false;
                }
                std::path::Component::CurDir => {} // skip "."
                other => normalised.push(other),
            }
        }

        // The normalised path must be inside the workspace.
        // Use the canonical workspace when available so symlink configs
        // don't cause false negatives.
        let ws_root = workspace_dir
            .canonicalize()
            .unwrap_or_else(|_| workspace_dir.to_path_buf());

        if !normalised.starts_with(&ws_root) {
            return false;
        }
    }

    true
}

impl SecurityPolicy {
    // ── Risk Classification ──────────────────────────────────────────────
    // Risk is assessed per-segment (split on shell operators), and the
    // highest risk across all segments wins. This prevents bypasses like
    // `ls && rm -rf /` from being classified as Low just because `ls` is safe.

    /// Classify command risk. Any high-risk segment marks the whole command high.
    pub fn command_risk_level(&self, command: &str) -> CommandRiskLevel {
        let mut saw_medium = false;

        for segment in split_unquoted_segments(command) {
            let cmd_part = skip_env_assignments(&segment);
            let mut words = cmd_part.split_whitespace();
            let Some(base_raw) = words.next() else {
                continue;
            };

            // Handle both Unix (/) and Windows (\) path separators
            let normalized = base_raw.replace('\\', "/");
            let base = normalized
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();

            let args: Vec<String> = words.map(|w| w.to_ascii_lowercase()).collect();
            let joined_segment = cmd_part.to_ascii_lowercase();

            // High-risk commands
            if matches!(
                base.as_str(),
                "rm" | "mkfs"
                    | "dd"
                    | "shutdown"
                    | "reboot"
                    | "halt"
                    | "poweroff"
                    | "sudo"
                    | "su"
                    | "chown"
                    | "chmod"
                    | "useradd"
                    | "userdel"
                    | "usermod"
                    | "passwd"
                    | "mount"
                    | "umount"
                    | "iptables"
                    | "ufw"
                    | "firewall-cmd"
                    | "wget"
                    | "nc"
                    | "ncat"
                    | "netcat"
                    | "scp"
                    | "ssh"
                    | "ftp"
                    | "telnet"
            ) {
                return CommandRiskLevel::High;
            }

            if joined_segment.contains("rm -rf /")
                || joined_segment.contains("rm -fr /")
                || joined_segment.contains(":(){:|:&};:")
            {
                return CommandRiskLevel::High;
            }

            // Medium-risk commands (state-changing, but not inherently destructive)
            let medium = match base.as_str() {
                "git" => args.first().is_some_and(|verb| {
                    matches!(
                        verb.as_str(),
                        "commit"
                            | "push"
                            | "reset"
                            | "clean"
                            | "rebase"
                            | "merge"
                            | "cherry-pick"
                            | "revert"
                            | "branch"
                            | "checkout"
                            | "switch"
                            | "tag"
                    )
                }),
                "npm" | "pnpm" | "yarn" => args.first().is_some_and(|verb| {
                    matches!(
                        verb.as_str(),
                        "install" | "add" | "remove" | "uninstall" | "update" | "publish"
                    )
                }),
                "cargo" => args.first().is_some_and(|verb| {
                    matches!(
                        verb.as_str(),
                        "add" | "remove" | "install" | "clean" | "publish"
                    )
                }),
                "curl" => true,  // Network requests - medium risk but useful for MCP tools
                "python" | "python3" | "python.exe" | "python3.exe" => true,  // Code execution - medium risk
                "touch" | "mkdir" | "mv" | "cp" | "ln" => true,
                _ => false,
            };

            saw_medium |= medium;
        }

        if saw_medium {
            CommandRiskLevel::Medium
        } else {
            CommandRiskLevel::Low
        }
    }

    // ── Command Execution Policy Gate ──────────────────────────────────────
    // Validation follows a strict precedence order:
    //   1. Allowlist check (is the base command permitted at all?)
    //   2. Risk classification (high / medium / low)
    //   3. Policy flags (block_high_risk_commands, require_approval_for_medium_risk)
    //   4. Autonomy level × approval status (supervised requires explicit approval)
    // This ordering ensures deny-by-default: unknown commands are rejected
    // before any risk or autonomy logic runs.

    /// Validate full command execution policy (allowlist + risk gate).
    pub fn validate_command_execution(
        &self,
        command: &str,
        approved: bool,
    ) -> Result<CommandRiskLevel, String> {
        if !self.is_command_allowed(command) {
            return Err(format!("Command not allowed by security policy: {command}"));
        }

        let risk = self.command_risk_level(command);

        if risk == CommandRiskLevel::High {
            if self.block_high_risk_commands {
                return Err("Command blocked: high-risk command is disallowed by policy".into());
            }
            if self.autonomy == AutonomyLevel::Supervised && !approved {
                return Err(
                    "Command requires explicit approval (approved=true): high-risk operation"
                        .into(),
                );
            }
        }

        if risk == CommandRiskLevel::Medium
            && self.autonomy == AutonomyLevel::Supervised
            && self.require_approval_for_medium_risk
            && !approved
        {
            return Err(
                "Command requires explicit approval (approved=true): medium-risk operation".into(),
            );
        }

        Ok(risk)
    }

    // ── Layered Command Allowlist ──────────────────────────────────────────
    // Defence-in-depth: five independent gates run in order before the
    // per-segment allowlist check. Each gate targets a specific bypass
    // technique. If any gate rejects, the whole command is blocked.

    /// Check if a shell command is allowed.
    ///
    /// Validates the **entire** command string, not just the first word:
    /// - Blocks subshell operators (`` ` ``, `$(`) that hide arbitrary execution
    /// - Splits on command separators (`|`, `&&`, `||`, `;`, newlines) and
    ///   validates each sub-command against the allowlist
    /// - Blocks single `&` background chaining (`&&` remains supported)
    /// - Blocks output redirections (`>`, `>>`) that could write outside workspace
    /// - Blocks dangerous arguments (e.g. `find -exec`, `git config`)
    pub fn is_command_allowed(&self, command: &str) -> bool {
        if self.autonomy == AutonomyLevel::ReadOnly {
            return false;
        }

        // Block subshell/expansion operators — these allow hiding arbitrary
        // commands inside an allowed command (e.g. `echo $(rm -rf /)`)
        if command.contains('`')
            || command.contains("$(")
            || command.contains("${")
            || command.contains("<(")
            || command.contains(">(")
        {
            return false;
        }

        // ── Redirect validation ──────────────────────────────────────────
        // Instead of blanket-blocking all `>` / `>>` / `<`, we extract the
        // redirect targets and verify every one resolves inside workspace_dir.
        // Quoted literals (e.g. `echo "a>b"`) are handled by the quote-aware
        // parser and never produce false-positive targets.
        if contains_unquoted_char(command, '>') || contains_unquoted_char(command, '<') {
            let targets = extract_redirect_targets(command);
            if targets.is_empty() {
                // Redirect operator present but no target — syntax error; block
                return false;
            }
            if !are_redirects_safe(&targets, &self.workspace_dir) {
                return false;
            }
        }

        // Block `tee` — it can write to arbitrary files, bypassing the
        // redirect check above (e.g. `echo secret | tee /etc/crontab`)
        if command
            .split_whitespace()
            .any(|w| w == "tee" || w.ends_with("/tee"))
        {
            return false;
        }

        // Block background command chaining (`&`), which can hide extra
        // sub-commands and outlive timeout expectations. Keep `&&` allowed.
        if contains_unquoted_single_ampersand(command) {
            return false;
        }

        // Split on unquoted command separators and validate each sub-command.
        let segments = split_unquoted_segments(command);
        for segment in &segments {
            // Strip leading env var assignments (e.g. FOO=bar cmd)
            let cmd_part = skip_env_assignments(segment);

            let mut words = cmd_part.split_whitespace();
            let base_raw = words.next().unwrap_or("");

            // Handle both Unix (/) and Windows (\) path separators
            // Replace backslashes with forward slashes for consistent parsing
            let normalized = base_raw.replace('\\', "/");
            let base_cmd = normalized.rsplit('/').next().unwrap_or("").to_ascii_lowercase();

            if base_cmd.is_empty() {
                continue;
            }

            if !self
                .allowed_commands
                .iter()
                .any(|allowed| allowed.to_ascii_lowercase() == base_cmd)
            {
                return false;
            }

            // Validate arguments for the command
            let args: Vec<String> = words.map(|w| w.to_ascii_lowercase()).collect();
            if !self.is_args_safe(&base_cmd, &args) {
                return false;
            }
        }

        // At least one command must be present
        let has_cmd = segments.iter().any(|s| {
            let s = skip_env_assignments(s.trim());
            s.split_whitespace().next().is_some_and(|w| !w.is_empty())
        });

        has_cmd
    }

    /// Check for dangerous arguments that allow sub-command execution.
    fn is_args_safe(&self, base: &str, args: &[String]) -> bool {
        let base = base.to_ascii_lowercase();
        match base.as_str() {
            "find" => {
                // find -exec and find -ok allow arbitrary command execution
                !args.iter().any(|arg| arg == "-exec" || arg == "-ok")
            }
            "git" => {
                // git config, alias, and -c can be used to set dangerous options
                // (e.g. git config core.editor "rm -rf /")
                !args.iter().any(|arg| {
                    arg == "config"
                        || arg.starts_with("config.")
                        || arg == "alias"
                        || arg.starts_with("alias.")
                        || arg == "-c"
                })
            }
            _ => true,
        }
    }

    // ── Path Validation ────────────────────────────────────────────────
    // Layered checks: null-byte injection → component-level traversal →
    // URL-encoded traversal → tilde expansion → absolute-path block →
    // forbidden-prefix match. Each layer addresses a distinct escape
    // technique; together they enforce workspace confinement.

    /// Check if a file path is allowed (no path traversal, within workspace)
    pub fn is_path_allowed(&self, path: &str) -> bool {
        // Block null bytes (can truncate paths in C-backed syscalls)
        if path.contains('\0') {
            return false;
        }

        // Block path traversal: check for ".." as a path component
        if Path::new(path)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return false;
        }

        // Block URL-encoded traversal attempts (e.g. ..%2f)
        let lower = path.to_lowercase();
        if lower.contains("..%2f") || lower.contains("%2f..") {
            return false;
        }

        // Expand tilde for comparison
        let expanded = if let Some(stripped) = path.strip_prefix("~/") {
            if let Some(home) = std::env::var("HOME").ok().map(PathBuf::from) {
                home.join(stripped).to_string_lossy().to_string()
            } else {
                path.to_string()
            }
        } else {
            path.to_string()
        };

        // Block absolute paths when workspace_only is set
        if self.workspace_only && Path::new(&expanded).is_absolute() {
            return false;
        }

        // Block forbidden paths using path-component-aware matching
        let expanded_path = Path::new(&expanded);
        for forbidden in &self.forbidden_paths {
            let forbidden_expanded = if let Some(stripped) = forbidden.strip_prefix("~/") {
                if let Some(home) = std::env::var("HOME").ok().map(PathBuf::from) {
                    home.join(stripped).to_string_lossy().to_string()
                } else {
                    forbidden.clone()
                }
            } else {
                forbidden.clone()
            };
            let forbidden_path = Path::new(&forbidden_expanded);
            if expanded_path.starts_with(forbidden_path) {
                return false;
            }
        }

        true
    }

    /// Validate that a resolved path is still inside the workspace.
    /// Call this AFTER joining `workspace_dir` + relative path and canonicalizing.
    pub fn is_resolved_path_allowed(&self, resolved: &Path) -> bool {
        // Must be under workspace_dir (prevents symlink escapes).
        // Prefer canonical workspace root so `/a/../b` style config paths don't
        // cause false positives or negatives.
        let workspace_root = self
            .workspace_dir
            .canonicalize()
            .unwrap_or_else(|_| self.workspace_dir.clone());
        resolved.starts_with(workspace_root)
    }

    /// Check if autonomy level permits any action at all
    pub fn can_act(&self) -> bool {
        self.autonomy != AutonomyLevel::ReadOnly
    }

    // ── Tool Operation Gating ──────────────────────────────────────────────
    // Read operations bypass autonomy and rate checks because they have
    // no side effects. Act operations must pass both the autonomy gate
    // (not read-only) and the sliding-window rate limiter.

    /// Enforce policy for a tool operation.
    ///
    /// Read operations are always allowed by autonomy/rate gates.
    /// Act operations require non-readonly autonomy and available action budget.
    pub fn enforce_tool_operation(
        &self,
        operation: ToolOperation,
        operation_name: &str,
    ) -> Result<(), String> {
        match operation {
            ToolOperation::Read => Ok(()),
            ToolOperation::Act => {
                if !self.can_act() {
                    return Err(format!(
                        "Security policy: read-only mode, cannot perform '{operation_name}'"
                    ));
                }

                if !self.record_action() {
                    return Err("Rate limit exceeded: action budget exhausted".to_string());
                }

                Ok(())
            }
        }
    }

    /// Record an action and check if the rate limit has been exceeded.
    /// Returns `true` if the action is allowed, `false` if rate-limited.
    pub fn record_action(&self) -> bool {
        let count = self.tracker.record();
        count <= self.max_actions_per_hour as usize
    }

    /// Check if the rate limit would be exceeded without recording.
    pub fn is_rate_limited(&self) -> bool {
        self.tracker.count() >= self.max_actions_per_hour as usize
    }

    /// Build from config sections
    pub fn from_config(
        autonomy_config: &crate::config::AutonomyConfig,
        workspace_dir: &Path,
    ) -> Self {
        Self {
            autonomy: autonomy_config.level,
            workspace_dir: workspace_dir.to_path_buf(),
            workspace_only: autonomy_config.workspace_only,
            allowed_commands: autonomy_config.allowed_commands.clone(),
            forbidden_paths: autonomy_config.forbidden_paths.clone(),
            max_actions_per_hour: autonomy_config.max_actions_per_hour,
            max_cost_per_day_cents: autonomy_config.max_cost_per_day_cents,
            require_approval_for_medium_risk: autonomy_config.require_approval_for_medium_risk,
            block_high_risk_commands: autonomy_config.block_high_risk_commands,
            tracker: ActionTracker::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_policy() -> SecurityPolicy {
        SecurityPolicy::default()
    }

    fn readonly_policy() -> SecurityPolicy {
        SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            ..SecurityPolicy::default()
        }
    }

    fn full_policy() -> SecurityPolicy {
        SecurityPolicy {
            autonomy: AutonomyLevel::Full,
            ..SecurityPolicy::default()
        }
    }

    // ── AutonomyLevel ────────────────────────────────────────

    #[test]
    fn autonomy_default_is_supervised() {
        assert_eq!(AutonomyLevel::default(), AutonomyLevel::Supervised);
    }

    #[test]
    fn autonomy_serde_roundtrip() {
        let json = serde_json::to_string(&AutonomyLevel::Full).unwrap();
        assert_eq!(json, "\"full\"");
        let parsed: AutonomyLevel = serde_json::from_str("\"readonly\"").unwrap();
        assert_eq!(parsed, AutonomyLevel::ReadOnly);
        let parsed2: AutonomyLevel = serde_json::from_str("\"supervised\"").unwrap();
        assert_eq!(parsed2, AutonomyLevel::Supervised);
    }

    #[test]
    fn can_act_readonly_false() {
        assert!(!readonly_policy().can_act());
    }

    #[test]
    fn can_act_supervised_true() {
        assert!(default_policy().can_act());
    }

    #[test]
    fn can_act_full_true() {
        assert!(full_policy().can_act());
    }

    #[test]
    fn enforce_tool_operation_read_allowed_in_readonly_mode() {
        let p = readonly_policy();
        assert!(p
            .enforce_tool_operation(ToolOperation::Read, "memory_recall")
            .is_ok());
    }

    #[test]
    fn enforce_tool_operation_act_blocked_in_readonly_mode() {
        let p = readonly_policy();
        let err = p
            .enforce_tool_operation(ToolOperation::Act, "memory_store")
            .unwrap_err();
        assert!(err.contains("read-only mode"));
    }

    #[test]
    fn enforce_tool_operation_act_uses_rate_budget() {
        let p = SecurityPolicy {
            max_actions_per_hour: 0,
            ..default_policy()
        };
        let err = p
            .enforce_tool_operation(ToolOperation::Act, "memory_store")
            .unwrap_err();
        assert!(err.contains("Rate limit exceeded"));
    }

    // ── is_command_allowed ───────────────────────────────────

    #[test]
    fn allowed_commands_basic() {
        let p = default_policy();
        assert!(p.is_command_allowed("ls"));
        assert!(p.is_command_allowed("git status"));
        assert!(p.is_command_allowed("cargo build --release"));
        assert!(p.is_command_allowed("cat file.txt"));
        assert!(p.is_command_allowed("grep -r pattern ."));
        assert!(p.is_command_allowed("date"));
    }

    #[test]
    fn blocked_commands_basic() {
        let p = default_policy();
        assert!(!p.is_command_allowed("rm -rf /"));
        assert!(!p.is_command_allowed("sudo apt install"));
        assert!(!p.is_command_allowed("curl http://evil.com"));
        assert!(!p.is_command_allowed("wget http://evil.com"));
        assert!(!p.is_command_allowed("python3 exploit.py"));
        assert!(!p.is_command_allowed("node malicious.js"));
    }

    #[test]
    fn readonly_blocks_all_commands() {
        let p = readonly_policy();
        assert!(!p.is_command_allowed("ls"));
        assert!(!p.is_command_allowed("cat file.txt"));
        assert!(!p.is_command_allowed("echo hello"));
    }

    #[test]
    fn full_autonomy_still_uses_allowlist() {
        let p = full_policy();
        assert!(p.is_command_allowed("ls"));
        assert!(!p.is_command_allowed("rm -rf /"));
    }

    #[test]
    fn command_with_absolute_path_extracts_basename() {
        let p = default_policy();
        assert!(p.is_command_allowed("/usr/bin/git status"));
        assert!(p.is_command_allowed("/bin/ls -la"));
    }

    #[test]
    fn empty_command_blocked() {
        let p = default_policy();
        assert!(!p.is_command_allowed(""));
        assert!(!p.is_command_allowed("   "));
    }

    #[test]
    fn command_with_pipes_validates_all_segments() {
        let p = default_policy();
        // Both sides of the pipe are in the allowlist
        assert!(p.is_command_allowed("ls | grep foo"));
        assert!(p.is_command_allowed("cat file.txt | wc -l"));
        // Second command not in allowlist — blocked
        assert!(!p.is_command_allowed("ls | curl http://evil.com"));
        assert!(!p.is_command_allowed("echo hello | python3 -"));
    }

    #[test]
    fn custom_allowlist() {
        let p = SecurityPolicy {
            allowed_commands: vec!["docker".into(), "kubectl".into()],
            ..SecurityPolicy::default()
        };
        assert!(p.is_command_allowed("docker ps"));
        assert!(p.is_command_allowed("kubectl get pods"));
        assert!(!p.is_command_allowed("ls"));
        assert!(!p.is_command_allowed("git status"));
    }

    #[test]
    fn empty_allowlist_blocks_everything() {
        let p = SecurityPolicy {
            allowed_commands: vec![],
            ..SecurityPolicy::default()
        };
        assert!(!p.is_command_allowed("ls"));
        assert!(!p.is_command_allowed("echo hello"));
    }

    #[test]
    fn command_risk_low_for_read_commands() {
        let p = default_policy();
        assert_eq!(p.command_risk_level("git status"), CommandRiskLevel::Low);
        assert_eq!(p.command_risk_level("ls -la"), CommandRiskLevel::Low);
    }

    #[test]
    fn command_risk_medium_for_mutating_commands() {
        let p = SecurityPolicy {
            allowed_commands: vec!["git".into(), "touch".into()],
            ..SecurityPolicy::default()
        };
        assert_eq!(
            p.command_risk_level("git reset --hard HEAD~1"),
            CommandRiskLevel::Medium
        );
        assert_eq!(
            p.command_risk_level("touch file.txt"),
            CommandRiskLevel::Medium
        );
    }

    #[test]
    fn command_risk_high_for_dangerous_commands() {
        let p = SecurityPolicy {
            allowed_commands: vec!["rm".into()],
            ..SecurityPolicy::default()
        };
        assert_eq!(
            p.command_risk_level("rm -rf /tmp/test"),
            CommandRiskLevel::High
        );
    }

    #[test]
    fn validate_command_requires_approval_for_medium_risk() {
        let p = SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            require_approval_for_medium_risk: true,
            allowed_commands: vec!["touch".into()],
            ..SecurityPolicy::default()
        };

        let denied = p.validate_command_execution("touch test.txt", false);
        assert!(denied.is_err());
        assert!(denied.unwrap_err().contains("requires explicit approval"),);

        let allowed = p.validate_command_execution("touch test.txt", true);
        assert_eq!(allowed.unwrap(), CommandRiskLevel::Medium);
    }

    #[test]
    fn validate_command_blocks_high_risk_by_default() {
        let p = SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            allowed_commands: vec!["rm".into()],
            ..SecurityPolicy::default()
        };

        let result = p.validate_command_execution("rm -rf /tmp/test", true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("high-risk"));
    }

    #[test]
    fn validate_command_full_mode_skips_medium_risk_approval_gate() {
        let p = SecurityPolicy {
            autonomy: AutonomyLevel::Full,
            require_approval_for_medium_risk: true,
            allowed_commands: vec!["touch".into()],
            ..SecurityPolicy::default()
        };

        let result = p.validate_command_execution("touch test.txt", false);
        assert_eq!(result.unwrap(), CommandRiskLevel::Medium);
    }

    #[test]
    fn validate_command_rejects_background_chain_bypass() {
        let p = default_policy();
        let result = p.validate_command_execution("ls & python3 -c 'print(1)'", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not allowed"));
    }

    // ── is_path_allowed ─────────────────────────────────────

    #[test]
    fn relative_paths_allowed() {
        let p = default_policy();
        assert!(p.is_path_allowed("file.txt"));
        assert!(p.is_path_allowed("src/main.rs"));
        assert!(p.is_path_allowed("deep/nested/dir/file.txt"));
    }

    #[test]
    fn path_traversal_blocked() {
        let p = default_policy();
        assert!(!p.is_path_allowed("../etc/passwd"));
        assert!(!p.is_path_allowed("../../root/.ssh/id_rsa"));
        assert!(!p.is_path_allowed("foo/../../../etc/shadow"));
        assert!(!p.is_path_allowed(".."));
    }

    #[test]
    fn absolute_paths_blocked_when_workspace_only() {
        let p = default_policy();
        assert!(!p.is_path_allowed("/etc/passwd"));
        assert!(!p.is_path_allowed("/root/.ssh/id_rsa"));
        assert!(!p.is_path_allowed("/tmp/file.txt"));
    }

    #[test]
    fn absolute_paths_allowed_when_not_workspace_only() {
        let p = SecurityPolicy {
            workspace_only: false,
            forbidden_paths: vec![],
            ..SecurityPolicy::default()
        };
        assert!(p.is_path_allowed("/tmp/file.txt"));
    }

    #[test]
    fn forbidden_paths_blocked() {
        let p = SecurityPolicy {
            workspace_only: false,
            ..SecurityPolicy::default()
        };
        assert!(!p.is_path_allowed("/etc/passwd"));
        assert!(!p.is_path_allowed("/root/.bashrc"));
        assert!(!p.is_path_allowed("~/.ssh/id_rsa"));
        assert!(!p.is_path_allowed("~/.gnupg/pubring.kbx"));
    }

    #[test]
    fn empty_path_allowed() {
        let p = default_policy();
        assert!(p.is_path_allowed(""));
    }

    #[test]
    fn dotfile_in_workspace_allowed() {
        let p = default_policy();
        assert!(p.is_path_allowed(".gitignore"));
        assert!(p.is_path_allowed(".env"));
    }

    // ── from_config ─────────────────────────────────────────

    #[test]
    fn from_config_maps_all_fields() {
        let autonomy_config = crate::config::AutonomyConfig {
            level: AutonomyLevel::Full,
            workspace_only: false,
            allowed_commands: vec!["docker".into()],
            forbidden_paths: vec!["/secret".into()],
            max_actions_per_hour: 100,
            max_cost_per_day_cents: 1000,
            require_approval_for_medium_risk: false,
            block_high_risk_commands: false,
            ..crate::config::AutonomyConfig::default()
        };
        let workspace = PathBuf::from("/tmp/test-workspace");
        let policy = SecurityPolicy::from_config(&autonomy_config, &workspace);

        assert_eq!(policy.autonomy, AutonomyLevel::Full);
        assert!(!policy.workspace_only);
        assert_eq!(policy.allowed_commands, vec!["docker"]);
        assert_eq!(policy.forbidden_paths, vec!["/secret"]);
        assert_eq!(policy.max_actions_per_hour, 100);
        assert_eq!(policy.max_cost_per_day_cents, 1000);
        assert!(!policy.require_approval_for_medium_risk);
        assert!(!policy.block_high_risk_commands);
        assert_eq!(policy.workspace_dir, PathBuf::from("/tmp/test-workspace"));
    }

    // ── Default policy ──────────────────────────────────────

    #[test]
    fn default_policy_has_sane_values() {
        let p = SecurityPolicy::default();
        assert_eq!(p.autonomy, AutonomyLevel::Supervised);
        assert!(p.workspace_only);
        assert!(!p.allowed_commands.is_empty());
        assert!(!p.forbidden_paths.is_empty());
        assert!(p.max_actions_per_hour > 0);
        assert!(p.max_cost_per_day_cents > 0);
        assert!(p.require_approval_for_medium_risk);
        assert!(p.block_high_risk_commands);
    }

    // ── ActionTracker / rate limiting ───────────────────────

    #[test]
    fn action_tracker_starts_at_zero() {
        let tracker = ActionTracker::new();
        assert_eq!(tracker.count(), 0);
    }

    #[test]
    fn action_tracker_records_actions() {
        let tracker = ActionTracker::new();
        assert_eq!(tracker.record(), 1);
        assert_eq!(tracker.record(), 2);
        assert_eq!(tracker.record(), 3);
        assert_eq!(tracker.count(), 3);
    }

    #[test]
    fn record_action_allows_within_limit() {
        let p = SecurityPolicy {
            max_actions_per_hour: 5,
            ..SecurityPolicy::default()
        };
        for _ in 0..5 {
            assert!(p.record_action(), "should allow actions within limit");
        }
    }

    #[test]
    fn record_action_blocks_over_limit() {
        let p = SecurityPolicy {
            max_actions_per_hour: 3,
            ..SecurityPolicy::default()
        };
        assert!(p.record_action()); // 1
        assert!(p.record_action()); // 2
        assert!(p.record_action()); // 3
        assert!(!p.record_action()); // 4 — over limit
    }

    #[test]
    fn is_rate_limited_reflects_count() {
        let p = SecurityPolicy {
            max_actions_per_hour: 2,
            ..SecurityPolicy::default()
        };
        assert!(!p.is_rate_limited());
        p.record_action();
        assert!(!p.is_rate_limited());
        p.record_action();
        assert!(p.is_rate_limited());
    }

    #[test]
    fn action_tracker_clone_is_independent() {
        let tracker = ActionTracker::new();
        tracker.record();
        tracker.record();
        let cloned = tracker.clone();
        assert_eq!(cloned.count(), 2);
        tracker.record();
        assert_eq!(tracker.count(), 3);
        assert_eq!(cloned.count(), 2); // clone is independent
    }

    // ── Edge cases: command injection ────────────────────────

    #[test]
    fn command_injection_semicolon_blocked() {
        let p = default_policy();
        // First word is "ls;" (with semicolon) — doesn't match "ls" in allowlist.
        // This is a safe default: chained commands are blocked.
        assert!(!p.is_command_allowed("ls; rm -rf /"));
    }

    #[test]
    fn command_injection_semicolon_no_space() {
        let p = default_policy();
        assert!(!p.is_command_allowed("ls;rm -rf /"));
    }

    #[test]
    fn quoted_semicolons_do_not_split_sqlite_command() {
        let p = SecurityPolicy {
            allowed_commands: vec!["sqlite3".into()],
            ..SecurityPolicy::default()
        };
        assert!(p.is_command_allowed(
            "sqlite3 /tmp/test.db \"CREATE TABLE t(id INT); INSERT INTO t VALUES(1); SELECT * FROM t;\""
        ));
        assert_eq!(
            p.command_risk_level(
                "sqlite3 /tmp/test.db \"CREATE TABLE t(id INT); INSERT INTO t VALUES(1); SELECT * FROM t;\""
            ),
            CommandRiskLevel::Low
        );
    }

    #[test]
    fn unquoted_semicolon_after_quoted_sql_still_splits_commands() {
        let p = SecurityPolicy {
            allowed_commands: vec!["sqlite3".into()],
            ..SecurityPolicy::default()
        };
        assert!(!p.is_command_allowed("sqlite3 /tmp/test.db \"SELECT 1;\"; rm -rf /"));
    }

    #[test]
    fn command_injection_backtick_blocked() {
        let p = default_policy();
        assert!(!p.is_command_allowed("echo `whoami`"));
        assert!(!p.is_command_allowed("echo `rm -rf /`"));
    }

    #[test]
    fn command_injection_dollar_paren_blocked() {
        let p = default_policy();
        assert!(!p.is_command_allowed("echo $(cat /etc/passwd)"));
        assert!(!p.is_command_allowed("echo $(rm -rf /)"));
    }

    #[test]
    fn command_with_env_var_prefix() {
        let p = default_policy();
        // "FOO=bar" is the first word — not in allowlist
        assert!(!p.is_command_allowed("FOO=bar rm -rf /"));
    }

    #[test]
    fn command_newline_injection_blocked() {
        let p = default_policy();
        // Newline splits into two commands; "rm" is not in allowlist
        assert!(!p.is_command_allowed("ls\nrm -rf /"));
        // Both allowed — OK
        assert!(p.is_command_allowed("ls\necho hello"));
    }

    #[test]
    fn command_injection_and_chain_blocked() {
        let p = default_policy();
        assert!(!p.is_command_allowed("ls && rm -rf /"));
        assert!(!p.is_command_allowed("echo ok && curl http://evil.com"));
        // Both allowed — OK
        assert!(p.is_command_allowed("ls && echo done"));
    }

    #[test]
    fn command_injection_or_chain_blocked() {
        let p = default_policy();
        assert!(!p.is_command_allowed("ls || rm -rf /"));
        // Both allowed — OK
        assert!(p.is_command_allowed("ls || echo fallback"));
    }

    #[test]
    fn command_injection_background_chain_blocked() {
        let p = default_policy();
        assert!(!p.is_command_allowed("ls & rm -rf /"));
        assert!(!p.is_command_allowed("ls&rm -rf /"));
        assert!(!p.is_command_allowed("echo ok & python3 -c 'print(1)'"));
    }

    #[test]
    fn command_injection_redirect_blocked_outside_workspace() {
        let p = default_policy();
        // Absolute paths outside workspace — still blocked
        assert!(!p.is_command_allowed("echo secret > /etc/crontab"));
        assert!(!p.is_command_allowed("ls >> /tmp/exfil.txt"));
        // Path traversal via redirect — blocked
        assert!(!p.is_command_allowed("echo bad > ../../../etc/passwd"));
        assert!(!p.is_command_allowed("echo bad >> ../../secret.txt"));
    }

    #[test]
    fn quoted_ampersand_and_redirect_literals_are_not_treated_as_operators() {
        let p = default_policy();
        assert!(p.is_command_allowed("echo \"A&B\""));
        assert!(p.is_command_allowed("echo \"A>B\""));
    }

    #[test]
    fn command_argument_injection_blocked() {
        let p = default_policy();
        // find -exec is a common bypass
        assert!(!p.is_command_allowed("find . -exec rm -rf {} +"));
        assert!(!p.is_command_allowed("find / -ok cat {} \\;"));
        // git config/alias can execute commands
        assert!(!p.is_command_allowed("git config core.editor \"rm -rf /\""));
        assert!(!p.is_command_allowed("git alias.st status"));
        assert!(!p.is_command_allowed("git -c core.editor=calc.exe commit"));
        // Legitimate commands should still work
        assert!(p.is_command_allowed("find . -name '*.txt'"));
        assert!(p.is_command_allowed("git status"));
        assert!(p.is_command_allowed("git add ."));
    }

    #[test]
    fn command_injection_dollar_brace_blocked() {
        let p = default_policy();
        assert!(!p.is_command_allowed("echo ${IFS}cat${IFS}/etc/passwd"));
    }

    #[test]
    fn command_injection_tee_blocked() {
        let p = default_policy();
        assert!(!p.is_command_allowed("echo secret | tee /etc/crontab"));
        assert!(!p.is_command_allowed("ls | /usr/bin/tee outfile"));
        assert!(!p.is_command_allowed("tee file.txt"));
    }

    #[test]
    fn command_injection_process_substitution_blocked() {
        let p = default_policy();
        assert!(!p.is_command_allowed("cat <(echo pwned)"));
        assert!(!p.is_command_allowed("ls >(cat /etc/passwd)"));
    }

    #[test]
    fn command_env_var_prefix_with_allowed_cmd() {
        let p = default_policy();
        // env assignment + allowed command — OK
        assert!(p.is_command_allowed("FOO=bar ls"));
        assert!(p.is_command_allowed("LANG=C grep pattern file"));
        // env assignment + disallowed command — blocked
        assert!(!p.is_command_allowed("FOO=bar rm -rf /"));
    }

    // ── Edge cases: path traversal ──────────────────────────

    #[test]
    fn path_traversal_encoded_dots() {
        let p = default_policy();
        // Literal ".." in path — always blocked
        assert!(!p.is_path_allowed("foo/..%2f..%2fetc/passwd"));
    }

    #[test]
    fn path_traversal_double_dot_in_filename() {
        let p = default_policy();
        // ".." in a filename (not a path component) is allowed
        assert!(p.is_path_allowed("my..file.txt"));
        // But actual traversal components are still blocked
        assert!(!p.is_path_allowed("../etc/passwd"));
        assert!(!p.is_path_allowed("foo/../etc/passwd"));
    }

    #[test]
    fn path_with_null_byte_blocked() {
        let p = default_policy();
        assert!(!p.is_path_allowed("file\0.txt"));
    }

    #[test]
    fn path_symlink_style_absolute() {
        let p = default_policy();
        assert!(!p.is_path_allowed("/proc/self/root/etc/passwd"));
    }

    #[test]
    fn path_home_tilde_ssh() {
        let p = SecurityPolicy {
            workspace_only: false,
            ..SecurityPolicy::default()
        };
        assert!(!p.is_path_allowed("~/.ssh/id_rsa"));
        assert!(!p.is_path_allowed("~/.gnupg/secring.gpg"));
    }

    #[test]
    fn path_var_run_blocked() {
        let p = SecurityPolicy {
            workspace_only: false,
            ..SecurityPolicy::default()
        };
        assert!(!p.is_path_allowed("/var/run/docker.sock"));
    }

    // ── Edge cases: rate limiter boundary ────────────────────

    #[test]
    fn rate_limit_exactly_at_boundary() {
        let p = SecurityPolicy {
            max_actions_per_hour: 1,
            ..SecurityPolicy::default()
        };
        assert!(p.record_action()); // 1 — exactly at limit
        assert!(!p.record_action()); // 2 — over
        assert!(!p.record_action()); // 3 — still over
    }

    #[test]
    fn rate_limit_zero_blocks_everything() {
        let p = SecurityPolicy {
            max_actions_per_hour: 0,
            ..SecurityPolicy::default()
        };
        assert!(!p.record_action());
    }

    #[test]
    fn rate_limit_high_allows_many() {
        let p = SecurityPolicy {
            max_actions_per_hour: 10000,
            ..SecurityPolicy::default()
        };
        for _ in 0..100 {
            assert!(p.record_action());
        }
    }

    // ── Edge cases: autonomy + command combos ────────────────

    #[test]
    fn readonly_blocks_even_safe_commands() {
        let p = SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            allowed_commands: vec!["ls".into(), "cat".into()],
            ..SecurityPolicy::default()
        };
        assert!(!p.is_command_allowed("ls"));
        assert!(!p.is_command_allowed("cat"));
        assert!(!p.can_act());
    }

    #[test]
    fn supervised_allows_listed_commands() {
        let p = SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            allowed_commands: vec!["git".into()],
            ..SecurityPolicy::default()
        };
        assert!(p.is_command_allowed("git status"));
        assert!(!p.is_command_allowed("docker ps"));
    }

    #[test]
    fn full_autonomy_still_respects_forbidden_paths() {
        let p = SecurityPolicy {
            autonomy: AutonomyLevel::Full,
            workspace_only: false,
            ..SecurityPolicy::default()
        };
        assert!(!p.is_path_allowed("/etc/shadow"));
        assert!(!p.is_path_allowed("/root/.bashrc"));
    }

    // ── Edge cases: from_config preserves tracker ────────────

    #[test]
    fn from_config_creates_fresh_tracker() {
        let autonomy_config = crate::config::AutonomyConfig {
            level: AutonomyLevel::Full,
            workspace_only: false,
            allowed_commands: vec![],
            forbidden_paths: vec![],
            max_actions_per_hour: 10,
            max_cost_per_day_cents: 100,
            require_approval_for_medium_risk: true,
            block_high_risk_commands: true,
            ..crate::config::AutonomyConfig::default()
        };
        let workspace = PathBuf::from("/tmp/test");
        let policy = SecurityPolicy::from_config(&autonomy_config, &workspace);
        assert_eq!(policy.tracker.count(), 0);
        assert!(!policy.is_rate_limited());
    }

    // ══════════════════════════════════════════════════════════
    // SECURITY CHECKLIST TESTS
    // Checklist: gateway not public, pairing required,
    //            filesystem scoped (no /), access via tunnel
    // ══════════════════════════════════════════════════════════

    // ── Checklist #3: Filesystem scoped (no /) ──────────────

    #[test]
    fn checklist_root_path_blocked() {
        let p = default_policy();
        assert!(!p.is_path_allowed("/"));
        assert!(!p.is_path_allowed("/anything"));
    }

    #[test]
    fn checklist_all_system_dirs_blocked() {
        let p = SecurityPolicy {
            workspace_only: false,
            ..SecurityPolicy::default()
        };
        for dir in [
            "/etc", "/root", "/home", "/usr", "/bin", "/sbin", "/lib", "/opt", "/boot", "/dev",
            "/proc", "/sys", "/var", "/tmp",
        ] {
            assert!(
                !p.is_path_allowed(dir),
                "System dir should be blocked: {dir}"
            );
            assert!(
                !p.is_path_allowed(&format!("{dir}/subpath")),
                "Subpath of system dir should be blocked: {dir}/subpath"
            );
        }
    }

    #[test]
    fn checklist_sensitive_dotfiles_blocked() {
        let p = SecurityPolicy {
            workspace_only: false,
            ..SecurityPolicy::default()
        };
        for path in [
            "~/.ssh/id_rsa",
            "~/.gnupg/secring.gpg",
            "~/.aws/credentials",
            "~/.config/secrets",
        ] {
            assert!(
                !p.is_path_allowed(path),
                "Sensitive dotfile should be blocked: {path}"
            );
        }
    }

    #[test]
    fn checklist_null_byte_injection_blocked() {
        let p = default_policy();
        assert!(!p.is_path_allowed("safe\0/../../../etc/passwd"));
        assert!(!p.is_path_allowed("\0"));
        assert!(!p.is_path_allowed("file\0"));
    }

    #[test]
    fn checklist_workspace_only_blocks_all_absolute() {
        let p = SecurityPolicy {
            workspace_only: true,
            ..SecurityPolicy::default()
        };
        assert!(!p.is_path_allowed("/any/absolute/path"));
        assert!(p.is_path_allowed("relative/path.txt"));
    }

    #[test]
    fn checklist_resolved_path_must_be_in_workspace() {
        let p = SecurityPolicy {
            workspace_dir: PathBuf::from("/home/user/project"),
            ..SecurityPolicy::default()
        };
        // Inside workspace — allowed
        assert!(p.is_resolved_path_allowed(Path::new("/home/user/project/src/main.rs")));
        // Outside workspace — blocked (symlink escape)
        assert!(!p.is_resolved_path_allowed(Path::new("/etc/passwd")));
        assert!(!p.is_resolved_path_allowed(Path::new("/home/user/other_project/file")));
        // Root — blocked
        assert!(!p.is_resolved_path_allowed(Path::new("/")));
    }

    #[test]
    fn checklist_default_policy_is_workspace_only() {
        let p = SecurityPolicy::default();
        assert!(
            p.workspace_only,
            "Default policy must be workspace_only=true"
        );
    }

    #[test]
    fn checklist_default_forbidden_paths_comprehensive() {
        let p = SecurityPolicy::default();
        // Must contain all critical system dirs
        for dir in ["/etc", "/root", "/proc", "/sys", "/dev", "/var", "/tmp"] {
            assert!(
                p.forbidden_paths.iter().any(|f| f == dir),
                "Default forbidden_paths must include {dir}"
            );
        }
        // Must contain sensitive dotfiles
        for dot in ["~/.ssh", "~/.gnupg", "~/.aws"] {
            assert!(
                p.forbidden_paths.iter().any(|f| f == dot),
                "Default forbidden_paths must include {dot}"
            );
        }
    }

    // ── §1.2 Path resolution / symlink bypass tests ──────────

    #[test]
    fn resolved_path_blocks_outside_workspace() {
        let workspace = std::env::temp_dir().join("zeroclaw_test_resolved_path");
        let _ = std::fs::create_dir_all(&workspace);

        // Use the canonicalized workspace so starts_with checks match
        let canonical_workspace = workspace
            .canonicalize()
            .unwrap_or_else(|_| workspace.clone());

        let policy = SecurityPolicy {
            workspace_dir: canonical_workspace.clone(),
            ..SecurityPolicy::default()
        };

        // A resolved path inside the workspace should be allowed
        let inside = canonical_workspace.join("subdir").join("file.txt");
        assert!(
            policy.is_resolved_path_allowed(&inside),
            "path inside workspace should be allowed"
        );

        // A resolved path outside the workspace should be blocked
        let canonical_temp = std::env::temp_dir()
            .canonicalize()
            .unwrap_or_else(|_| std::env::temp_dir());
        let outside = canonical_temp.join("outside_workspace_zeroclaw");
        assert!(
            !policy.is_resolved_path_allowed(&outside),
            "path outside workspace must be blocked"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn resolved_path_blocks_root_escape() {
        let policy = SecurityPolicy {
            workspace_dir: PathBuf::from("/home/zeroclaw_user/project"),
            ..SecurityPolicy::default()
        };

        assert!(
            !policy.is_resolved_path_allowed(Path::new("/etc/passwd")),
            "resolved path to /etc/passwd must be blocked"
        );
        assert!(
            !policy.is_resolved_path_allowed(Path::new("/root/.bashrc")),
            "resolved path to /root/.bashrc must be blocked"
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolved_path_blocks_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join("zeroclaw_test_symlink_escape");
        let workspace = root.join("workspace");
        let outside = root.join("outside_target");

        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        // Create a symlink inside workspace pointing outside
        let link_path = workspace.join("escape_link");
        symlink(&outside, &link_path).unwrap();

        let policy = SecurityPolicy {
            workspace_dir: workspace.clone(),
            ..SecurityPolicy::default()
        };

        // The resolved symlink target should be outside workspace
        let resolved = link_path.canonicalize().unwrap();
        assert!(
            !policy.is_resolved_path_allowed(&resolved),
            "symlink-resolved path outside workspace must be blocked"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn is_path_allowed_blocks_null_bytes() {
        let policy = default_policy();
        assert!(
            !policy.is_path_allowed("file\0.txt"),
            "paths with null bytes must be blocked"
        );
    }

    #[test]
    fn is_path_allowed_blocks_url_encoded_traversal() {
        let policy = default_policy();
        assert!(
            !policy.is_path_allowed("..%2fetc%2fpasswd"),
            "URL-encoded path traversal must be blocked"
        );
        assert!(
            !policy.is_path_allowed("subdir%2f..%2f..%2fetc"),
            "URL-encoded parent dir traversal must be blocked"
        );
    }

    // ══════════════════════════════════════════════════════════
    // PHASE 2: SAFE REDIRECT & SHELL CHAINING TESTS
    // These tests verify that shell operators (|, >, >>, &&, ||)
    // are permitted when all redirect targets resolve inside the
    // workspace directory, while unsafe targets remain blocked.
    // ══════════════════════════════════════════════════════════

    /// Helper: build a policy with a known absolute workspace dir so
    /// redirect-target validation is deterministic in tests.
    fn workspace_policy() -> SecurityPolicy {
        SecurityPolicy {
            workspace_dir: PathBuf::from("/zeroclaw-data/workspace"),
            ..SecurityPolicy::default()
        }
    }

    // ── extract_redirect_targets unit tests ─────────────────

    #[test]
    fn extract_redirect_targets_single_output() {
        let targets = extract_redirect_targets("echo hello > output.txt");
        assert_eq!(targets, vec!["output.txt"]);
    }

    #[test]
    fn extract_redirect_targets_append() {
        let targets = extract_redirect_targets("echo hello >> log.txt");
        assert_eq!(targets, vec!["log.txt"]);
    }

    #[test]
    fn extract_redirect_targets_input_redirect() {
        let targets = extract_redirect_targets("wc -l < input.txt");
        assert_eq!(targets, vec!["input.txt"]);
    }

    #[test]
    fn extract_redirect_targets_multiple() {
        let targets = extract_redirect_targets("cmd < in.txt > out.txt");
        assert_eq!(targets, vec!["in.txt", "out.txt"]);
    }

    #[test]
    fn extract_redirect_targets_quoted_not_extracted() {
        // The `>` inside quotes is not an operator
        let targets = extract_redirect_targets("echo \"a>b\"");
        assert!(targets.is_empty());
    }

    #[test]
    fn extract_redirect_targets_absolute_path() {
        let targets = extract_redirect_targets("echo x > /etc/crontab");
        assert_eq!(targets, vec!["/etc/crontab"]);
    }

    #[test]
    fn extract_redirect_targets_traversal_path() {
        let targets = extract_redirect_targets("echo x > ../../../etc/passwd");
        assert_eq!(targets, vec!["../../../etc/passwd"]);
    }

    // ── are_redirects_safe unit tests ────────────────────────

    #[test]
    fn redirects_safe_relative_in_workspace() {
        let ws = Path::new("/zeroclaw-data/workspace");
        assert!(are_redirects_safe(
            &["output.txt".to_string(), "subdir/log.txt".to_string()],
            ws,
        ));
    }

    #[test]
    fn redirects_unsafe_absolute_outside_workspace() {
        let ws = Path::new("/zeroclaw-data/workspace");
        assert!(!are_redirects_safe(&["/etc/passwd".to_string()], ws));
    }

    #[test]
    fn redirects_unsafe_traversal() {
        let ws = Path::new("/zeroclaw-data/workspace");
        assert!(!are_redirects_safe(
            &["../../../etc/passwd".to_string()],
            ws,
        ));
    }

    #[test]
    fn redirects_safe_absolute_inside_workspace() {
        let ws = Path::new("/zeroclaw-data/workspace");
        assert!(are_redirects_safe(
            &["/zeroclaw-data/workspace/out.txt".to_string()],
            ws,
        ));
    }

    #[test]
    fn redirects_unsafe_mixed() {
        let ws = Path::new("/zeroclaw-data/workspace");
        // One safe + one unsafe → overall unsafe
        assert!(!are_redirects_safe(
            &[
                "safe.txt".to_string(),
                "/etc/shadow".to_string(),
            ],
            ws,
        ));
    }

    // ── is_command_allowed: safe shell chaining ──────────────

    #[test]
    fn safe_redirect_to_workspace_allowed() {
        let p = workspace_policy();
        assert!(
            p.is_command_allowed("echo hello > /zeroclaw-data/workspace/output.txt"),
            "redirect to workspace absolute path should be allowed"
        );
    }

    #[test]
    fn safe_append_redirect_allowed() {
        let p = workspace_policy();
        assert!(
            p.is_command_allowed("echo hello >> /zeroclaw-data/workspace/log.txt"),
            "append redirect to workspace should be allowed"
        );
    }

    #[test]
    fn safe_pipe_with_redirect_allowed() {
        let p = workspace_policy();
        assert!(
            p.is_command_allowed("ls | grep foo > /zeroclaw-data/workspace/results.txt"),
            "pipe + redirect to workspace should be allowed"
        );
    }

    #[test]
    fn safe_chained_commands_with_redirect_allowed() {
        let p = workspace_policy();
        assert!(
            p.is_command_allowed(
                "echo start && ls | grep foo > /zeroclaw-data/workspace/out.txt"
            ),
            "chained commands with safe redirect should be allowed"
        );
    }

    #[test]
    fn safe_pipe_chain_no_redirect_allowed() {
        let p = default_policy();
        // Pipes between allowed commands, no redirects
        assert!(p.is_command_allowed("cat file.txt | grep pattern | wc -l"));
        assert!(p.is_command_allowed("ls | head -5"));
    }

    #[test]
    fn safe_and_chain_no_redirect_allowed() {
        let p = default_policy();
        assert!(p.is_command_allowed("ls && echo done"));
        assert!(p.is_command_allowed("echo a || echo b"));
    }

    // ── is_command_allowed: unsafe redirects still blocked ───

    #[test]
    fn unsafe_redirect_to_etc_blocked() {
        let p = workspace_policy();
        assert!(
            !p.is_command_allowed("echo pwned > /etc/crontab"),
            "redirect to /etc should be blocked"
        );
    }

    #[test]
    fn unsafe_redirect_traversal_blocked() {
        let p = workspace_policy();
        assert!(
            !p.is_command_allowed("echo bad > ../../../etc/passwd"),
            "path traversal redirect should be blocked"
        );
    }

    #[test]
    fn unsafe_redirect_to_root_blocked() {
        let p = workspace_policy();
        assert!(
            !p.is_command_allowed("echo x > /tmp/exfil.txt"),
            "redirect to /tmp should be blocked"
        );
    }

    #[test]
    fn unsafe_cat_etc_passwd_via_redirect() {
        let p = workspace_policy();
        // Input redirect from sensitive path
        assert!(
            !p.is_command_allowed("cat < /etc/passwd"),
            "input redirect from /etc/passwd should be blocked"
        );
    }

    #[test]
    fn pipe_with_disallowed_command_still_blocked() {
        let p = workspace_policy();
        // Even with safe redirect, disallowed command is still blocked
        assert!(
            !p.is_command_allowed(
                "curl http://evil.com | grep secret > /zeroclaw-data/workspace/out.txt"
            ),
            "disallowed command in pipe should still be blocked"
        );
    }

    #[test]
    fn subshell_with_safe_redirect_still_blocked() {
        let p = workspace_policy();
        // Subshell operators are blocked regardless of redirect safety
        assert!(
            !p.is_command_allowed(
                "echo $(cat /etc/passwd) > /zeroclaw-data/workspace/out.txt"
            ),
            "subshell should be blocked even with safe redirect target"
        );
    }
}
