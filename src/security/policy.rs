use parking_lot::Mutex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

const DEFAULT_WORKSPACE_ROOT: &str = "/zeroclaw-data/workspace";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AutonomyLevel {
    ReadOnly,
    #[default]
    Supervised,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityPolicyLevel {
    ReadOnly,
    Supervised,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    MalformedRedirect(String),
    DangerousRedirect(String),
    PathTraversal(String),
    UnauthorizedPath(String),
}

#[derive(Debug)]
pub struct ActionTracker {
    actions: Mutex<Vec<Instant>>,
}

impl ActionTracker {
    pub fn new() -> Self {
        Self {
            actions: Mutex::new(Vec::new()),
        }
    }

    pub fn record(&self) -> usize {
        let mut actions = self.actions.lock();
        let cutoff = Instant::now()
            .checked_sub(std::time::Duration::from_secs(3600))
            .unwrap_or_else(Instant::now);
        actions.retain(|t| *t > cutoff);
        actions.push(Instant::now());
        actions.len()
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteState {
    None,
    Single,
    Double,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            autonomy: AutonomyLevel::Supervised,
            workspace_dir: PathBuf::from(DEFAULT_WORKSPACE_ROOT),
            workspace_only: true,
            allowed_commands: vec![
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
                "sort".into(),
                "uniq".into(),
                "sed".into(),
                "awk".into(),
                "git".into(),
                "bash".into(),
                "sh".into(),
                "python3".into(),
                "pip".into(),
                "curl".into(),
            ],
            forbidden_paths: vec![
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
            ],
            max_actions_per_hour: 1000,
            max_cost_per_day_cents: 1000,
            require_approval_for_medium_risk: false,
            block_high_risk_commands: true,
            tracker: ActionTracker::new(),
        }
    }
}

fn skip_env_assignments(s: &str) -> &str {
    let mut rest = s.trim_start();
    loop {
        let Some(word) = rest.split_whitespace().next() else {
            return rest;
        };
        if word.contains('=')
            && word
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            rest = rest[word.len()..].trim_start();
        } else {
            return rest;
        }
    }
}

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
                    '|' => {
                        if chars.next_if_eq(&'|').is_some() {
                            push_segment(&mut segments, &mut current);
                        } else {
                            push_segment(&mut segments, &mut current);
                        }
                    }
                    '&' => {
                        if chars.next_if_eq(&'&').is_some() {
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

fn has_unquoted_char(command: &str, target: char) -> bool {
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

pub fn extract_redirect_targets(command: &str) -> Result<Vec<String>, PolicyError> {
    let mut targets = Vec::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape_next = false;
    let mut i = 0;
    let bytes = command.as_bytes();

    while i < bytes.len() {
        let ch = bytes[i] as char;

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

        if in_single_quote || in_double_quote {
            i += 1;
            continue;
        }

        if ch == '>' {
            let is_append = (i + 1 < bytes.len()) && (bytes[i + 1] as char == '>');
            let operator_len = if is_append { 2 } else { 1 };
            i += operator_len;

            while i < bytes.len() && (bytes[i] as char).is_whitespace() {
                i += 1;
            }

            if i >= bytes.len() {
                return Err(PolicyError::MalformedRedirect(
                    "No redirect target after > or >>".into(),
                ));
            }

            let mut target = String::new();

            if bytes[i] as char == '"' || bytes[i] as char == '\'' {
                let quote = bytes[i] as char;
                i += 1;
                while i < bytes.len() && (bytes[i] as char) != quote {
                    target.push(bytes[i] as char);
                    i += 1;
                }
                if i >= bytes.len() {
                    return Err(PolicyError::MalformedRedirect(
                        "Unclosed quoted redirect target".into(),
                    ));
                }
                i += 1;
            } else {
                while i < bytes.len() {
                    let c = bytes[i] as char;
                    if c.is_whitespace() || c == '|' || c == '&' || c == ';' {
                        break;
                    }
                    target.push(c);
                    i += 1;
                }
            }

            if target.is_empty() {
                return Err(PolicyError::MalformedRedirect(
                    "Redirect target cannot be empty".into(),
                ));
            }

            if target.starts_with('$') || target.starts_with('(') || target.starts_with('&') {
                return Err(PolicyError::DangerousRedirect(format!(
                    "Redirect target cannot be variable/substitution/fd alias: {target}"
                )));
            }

            targets.push(target);
            continue;
        }

        i += 1;
    }

    Ok(targets)
}

fn path_has_parent_traversal(path: &Path) -> bool {
    path.components().any(|c| matches!(c, Component::ParentDir))
}

fn workspace_canonical_fallback(workspace_dir: &Path) -> Result<PathBuf, PolicyError> {
    if workspace_dir.exists() {
        workspace_dir
            .canonicalize()
            .map_err(|e| PolicyError::PathTraversal(format!("Invalid workspace path: {e}")))
    } else {
        Ok(PathBuf::from(workspace_dir))
    }
}

pub fn are_redirects_safe(targets: &[String], workspace_dir: &Path) -> Result<bool, PolicyError> {
    let workspace_canonical = workspace_canonical_fallback(workspace_dir)?;

    let forbidden = ["/tmp", "/var", "/dev", "/etc", "/sys", "/proc", "/root"];

    for target in targets {
        if target.is_empty() {
            return Err(PolicyError::MalformedRedirect(
                "Redirect target cannot be empty".into(),
            ));
        }

        if target.contains('\0') || target.contains('~') {
            return Err(PolicyError::DangerousRedirect(format!(
                "Redirect target contains unsafe characters: {target}"
            )));
        }

        let target_path = Path::new(target);
        if path_has_parent_traversal(target_path) {
            return Err(PolicyError::PathTraversal(format!(
                "Path traversal in redirect target: {target}"
            )));
        }

        if target.starts_with('/') {
            for block in forbidden {
                if target.starts_with(block) {
                    return Err(PolicyError::UnauthorizedPath(format!(
                        "Redirect to forbidden path '{block}': {target}"
                    )));
                }
            }
        }

        let full_path = if target_path.is_absolute() {
            PathBuf::from(target)
        } else {
            workspace_canonical.join(target)
        };

        let resolved = if full_path.exists() {
            full_path.canonicalize().map_err(|e| {
                PolicyError::PathTraversal(format!(
                    "Failed to resolve redirect target '{target}': {e}"
                ))
            })?
        } else {
            let parent = full_path.parent().unwrap_or(&workspace_canonical);
            let parent_resolved = if parent.exists() {
                parent.canonicalize().map_err(|e| {
                    PolicyError::PathTraversal(format!(
                        "Cannot resolve parent directory '{}' for redirect target '{target}': {e}",
                        parent.display()
                    ))
                })?
            } else {
                parent.to_path_buf()
            };
            parent_resolved.join(
                full_path
                    .file_name()
                    .ok_or_else(|| {
                        PolicyError::MalformedRedirect(format!(
                            "Invalid redirect target filename: {target}"
                        ))
                    })?
                    .to_string_lossy()
                    .as_ref(),
            )
        };

        if !resolved.starts_with(&workspace_canonical) {
            return Err(PolicyError::UnauthorizedPath(format!(
                "Redirect target resolves outside workspace: {target} -> {}",
                resolved.display()
            )));
        }
    }

    Ok(true)
}

impl SecurityPolicy {
    pub fn command_risk_level(&self, command: &str) -> CommandRiskLevel {
        let mut saw_medium = false;

        for segment in split_unquoted_segments(command) {
            let cmd_part = skip_env_assignments(&segment);
            let mut words = cmd_part.split_whitespace();
            let Some(base_raw) = words.next() else {
                continue;
            };

            let normalized = base_raw.replace('\\', "/");
            let base = normalized
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            let args: Vec<String> = words.map(|w| w.to_ascii_lowercase()).collect();

            if matches!(
                base.as_str(),
                "rm"
                    | "mkfs"
                    | "dd"
                    | "shutdown"
                    | "reboot"
                    | "halt"
                    | "poweroff"
                    | "sudo"
                    | "su"
                    | "doas"
                    | "fdisk"
            ) {
                return CommandRiskLevel::High;
            }

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
                "npm" | "pnpm" | "yarn" | "pip" | "pip3" => true,
                "python" | "python3" | "bash" | "sh" => true,
                "curl" | "wget" => true,
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

    pub fn is_command_allowed(&self, command: &str) -> bool {
        if self.autonomy == AutonomyLevel::ReadOnly {
            return false;
        }

        let command = command.trim();
        if command.is_empty() {
            return false;
        }

        // Keep immutable hardcoded blocks.
        if command.contains('`')
            || command.contains("$(")
            || command.contains("${")
            || command.contains("<(")
            || command.contains(">(")
        {
            return false;
        }

        if has_unquoted_char(command, ';') || has_unquoted_char(command, '\n') {
            return false;
        }

        if contains_unquoted_single_ampersand(command) {
            return false;
        }

        if command
            .split_whitespace()
            .any(|w| w.eq_ignore_ascii_case("tee") || w.ends_with("/tee"))
        {
            return false;
        }

        let redirect_targets = match extract_redirect_targets(command) {
            Ok(t) => t,
            Err(_) => return false,
        };

        if !redirect_targets.is_empty()
            && are_redirects_safe(&redirect_targets, &self.workspace_dir).is_err()
        {
            return false;
        }

        let segments = split_unquoted_segments(command);
        if segments.is_empty() {
            return false;
        }

        for segment in &segments {
            let without_redirect = match segment.find('>') {
                Some(idx) => segment[..idx].trim(),
                None => segment.trim(),
            };
            let cmd_part = skip_env_assignments(without_redirect);
            let mut words = cmd_part.split_whitespace();
            let Some(base_raw) = words.next() else {
                return false;
            };

            let normalized = base_raw.replace('\\', "/");
            let base_cmd = normalized
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();

            if base_cmd.is_empty() {
                return false;
            }

            if !self
                .allowed_commands
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(&base_cmd))
            {
                return false;
            }

            let args: Vec<String> = words.map(|w| w.to_ascii_lowercase()).collect();
            if !self.is_args_safe(&base_cmd, &args) {
                return false;
            }
        }

        true
    }

    fn is_args_safe(&self, base: &str, args: &[String]) -> bool {
        match base {
            "find" => !args.iter().any(|arg| arg == "-exec" || arg == "-ok"),
            "git" => !args.iter().any(|arg| {
                arg == "config"
                    || arg.starts_with("config.")
                    || arg == "alias"
                    || arg.starts_with("alias.")
                    || arg == "-c"
            }),
            _ => true,
        }
    }

    pub fn is_path_allowed(&self, path: &str) -> bool {
        if path.contains('\0') {
            return false;
        }

        if Path::new(path)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return false;
        }

        let lower = path.to_lowercase();
        if lower.contains("..%2f") || lower.contains("%2f..") {
            return false;
        }

        let expanded = if let Some(stripped) = path.strip_prefix("~/") {
            if let Some(home) = std::env::var("HOME").ok().map(PathBuf::from) {
                home.join(stripped).to_string_lossy().to_string()
            } else {
                path.to_string()
            }
        } else {
            path.to_string()
        };

        if self.workspace_only && Path::new(&expanded).is_absolute() {
            return false;
        }

        let expanded_path = Path::new(&expanded);
        for forbidden in &self.forbidden_paths {
            let forbidden_path = Path::new(forbidden);
            if expanded_path.starts_with(forbidden_path) {
                return false;
            }
        }

        true
    }

    pub fn is_resolved_path_allowed(&self, resolved: &Path) -> bool {
        let workspace_root = self
            .workspace_dir
            .canonicalize()
            .unwrap_or_else(|_| self.workspace_dir.clone());
        resolved.starts_with(workspace_root)
    }

    pub fn can_act(&self) -> bool {
        self.autonomy != AutonomyLevel::ReadOnly
    }

    pub fn record_action(&self) -> bool {
        let count = self.tracker.record();
        count <= self.max_actions_per_hour as usize
    }

    pub fn is_rate_limited(&self) -> bool {
        self.tracker.count() >= self.max_actions_per_hour as usize
    }

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

    fn policy() -> SecurityPolicy {
        SecurityPolicy {
            workspace_dir: PathBuf::from("/zeroclaw-data/workspace"),
            allowed_commands: vec!["echo".into(), "cat".into(), "grep".into(), "find".into(), "git".into(), "pip".into()],
            ..SecurityPolicy::default()
        }
    }

    #[test]
    fn extract_redirect_targets_handles_quotes_and_append() {
        let targets = extract_redirect_targets("echo test > \"file one.txt\" >> log.txt").unwrap();
        assert_eq!(targets, vec!["file one.txt", "log.txt"]);
    }

    #[test]
    fn extract_redirect_targets_rejects_variable_target() {
        assert!(extract_redirect_targets("echo hi > $OUT").is_err());
    }

    #[test]
    fn redirects_must_stay_in_workspace() {
        let workspace = PathBuf::from("/zeroclaw-data/workspace");
        assert!(are_redirects_safe(&["logs/run.log".into()], &workspace).is_ok());
        assert!(are_redirects_safe(&["/etc/passwd".into()], &workspace).is_err());
    }

    #[test]
    fn allows_safe_pipe_and_redirect() {
        let p = policy();
        assert!(p.is_command_allowed("cat input.txt | grep ok > logs/out.txt"));
    }

    #[test]
    fn blocks_find_exec_and_git_config() {
        let p = policy();
        assert!(!p.is_command_allowed("find . -exec ls {} \\;"));
        assert!(!p.is_command_allowed("git config user.name bad"));
    }

    #[test]
    fn blocks_subshell_and_background() {
        let p = policy();
        assert!(!p.is_command_allowed("echo $(whoami)"));
        assert!(!p.is_command_allowed("echo hi & echo bye"));
    }
}
