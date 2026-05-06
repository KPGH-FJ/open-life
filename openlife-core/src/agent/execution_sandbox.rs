//! ExecutionSandbox — policy types and validation helpers for shell execution.
//!
//! No shell execution happens here. This module defines the sandbox policy
//! primitives that any future bash/shell executor must enforce.

use serde::{Deserialize, Serialize};

/// Network policy for sandboxed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    /// No network access allowed.
    None,
    /// Only loopback (localhost) allowed.
    LoopbackOnly,
    /// Full network access.
    Allowed,
}

/// File write policy for sandboxed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WritePolicy {
    /// All writes are proposal-first.
    ProposalFirst,
    /// No writes allowed.
    Denied,
    /// Writes allowed within safe_paths only.
    SafePathsOnly,
}

/// Sandbox policy for shell/bash execution.
///
/// All fields default to maximum safety: bash disabled, no network,
/// secure paths only, and strict command filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionSandbox {
    /// Working directory for execution.
    pub cwd: String,
    /// Paths that are safe for read and write.
    pub safe_paths: Vec<String>,
    /// Glob patterns for paths that should never be read.
    pub deny_read_patterns: Vec<String>,
    /// Glob patterns for paths that should never be written.
    pub deny_write_patterns: Vec<String>,
    /// Network access policy.
    pub network_policy: NetworkPolicy,
    /// Write/output policy.
    pub write_policy: WritePolicy,
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
    /// Maximum output size in bytes.
    pub max_output_bytes: usize,
    /// Environment variables allowed (name only, values from caller).
    pub env_allowlist: Vec<String>,
    /// Commands that are explicitly allowed.
    pub command_allowlist: Vec<String>,
    /// Commands that are permanently forbidden.
    pub dangerous_command_denylist: Vec<String>,
    /// Whether bash/shell execution is enabled at all.
    pub bash_enabled: bool,
}

impl Default for ExecutionSandbox {
    fn default() -> Self {
        Self {
            cwd: "/tmp".into(),
            safe_paths: vec!["/tmp".into(), "./".into()],
            deny_read_patterns: vec![
                "**/.env".into(),
                "**/.env.*".into(),
                "**/id_rsa".into(),
                "**/id_ed25519".into(),
                "**/*.pem".into(),
                "**/.ssh/**".into(),
                "**/.ssh".into(),
                "**/.aws/**".into(),
                "**/credentials".into(),
                "**/secrets/**".into(),
                "**/.git-credentials".into(),
            ],
            deny_write_patterns: vec![
                "**/.env".into(),
                "**/.git/**".into(),
                "**/node_modules/**".into(),
                "/etc/**".into(),
                "/System/**".into(),
            ],
            network_policy: NetworkPolicy::None,
            write_policy: WritePolicy::ProposalFirst,
            timeout_ms: 30_000,
            max_output_bytes: 1024 * 1024, // 1 MB
            env_allowlist: vec!["PATH".into(), "HOME".into(), "USER".into()],
            command_allowlist: vec![
                "ls".into(),
                "cat".into(),
                "head".into(),
                "tail".into(),
                "wc".into(),
                "grep".into(),
                "find".into(),
                "echo".into(),
                "date".into(),
                "pwd".into(),
                "whoami".into(),
                "uname".into(),
            ],
            dangerous_command_denylist: vec![
                "rm".into(),
                "mv".into(),
                "dd".into(),
                "mkfs".into(),
                "shutdown".into(),
                "reboot".into(),
                "kill".into(),
                "killall".into(),
                "sudo".into(),
                "su".into(),
                "chmod".into(),
                "chown".into(),
                "passwd".into(),
                "curl".into(),
                "wget".into(),
                "ssh".into(),
                "scp".into(),
                "nc".into(),
                "ncat".into(),
                "telnet".into(),
            ],
            bash_enabled: false,
        }
    }
}

impl ExecutionSandbox {
    /// Check whether a path is within the allowed safe_paths.
    pub fn is_path_in_safe_paths(&self, path: &str) -> bool {
        self.safe_paths.iter().any(|safe| path.starts_with(safe))
    }

    /// Check whether a path matches any deny_read pattern.
    pub fn is_path_denied_read(&self, path: &str) -> bool {
        self.deny_read_patterns
            .iter()
            .any(|pattern| glob_matches(pattern, path))
    }

    /// Check whether a path matches any deny_write pattern.
    pub fn is_path_denied_write(&self, path: &str) -> bool {
        self.deny_write_patterns
            .iter()
            .any(|pattern| glob_matches(pattern, path))
    }

    /// Check whether a command is in the allowlist.
    pub fn is_command_allowed(&self, command: &str) -> bool {
        let cmd_name = command.split_whitespace().next().unwrap_or(command);
        self.command_allowlist
            .iter()
            .any(|c| c == cmd_name)
    }

    /// Check whether a command is permanently forbidden.
    pub fn is_command_dangerous(&self, command: &str) -> bool {
        let cmd_name = command.split_whitespace().next().unwrap_or(command);
        self.dangerous_command_denylist
            .iter()
            .any(|c| c == cmd_name)
    }

    /// Check whether an environment variable name is in the allowlist.
    pub fn is_env_allowed(&self, var_name: &str) -> bool {
        self.env_allowlist.iter().any(|v| v == var_name)
    }

    /// Validate a command and path against the sandbox policy.
    /// Returns Ok(()) if allowed, Err(reason) if blocked.
    pub fn validate(&self, command: &str, cwd: Option<&str>) -> Result<(), String> {
        if !self.bash_enabled {
            return Err("bash execution is disabled".into());
        }

        if self.is_command_dangerous(command) {
            return Err(format!(
                "command '{}' is on the dangerous command denylist",
                command.split_whitespace().next().unwrap_or(command)
            ));
        }

        if !self.command_allowlist.is_empty() && !self.is_command_allowed(command) {
            return Err(format!(
                "command '{}' is not in the allowed command list",
                command.split_whitespace().next().unwrap_or(command)
            ));
        }

        if let Some(dir) = cwd {
            if self.is_path_denied_read(dir) {
                return Err(format!("working directory '{}' is denied (read block)", dir));
            }
            if self.is_path_denied_write(dir) {
                return Err(format!(
                    "working directory '{}' is denied (write block)",
                    dir
                ));
            }
        }

        Ok(())
    }
}

/// Simple glob matching. Supports `**` and `*` wildcards.
fn glob_matches(pattern: &str, path: &str) -> bool {
    if pattern == path {
        return true;
    }
    // Handle **/middle/** patterns (three segments)
    let parts: Vec<&str> = pattern.split("**").collect();
    if parts.len() == 3 && parts[0].is_empty() && parts[2].is_empty() {
        // Pattern like "**/middle/**" — middle must appear in path
        let middle = parts[1].trim_matches('/');
        if middle.contains('*') {
            // Fall through to segment-based matching
        } else {
            return path.split('/').any(|seg| seg == middle)
                || path.contains(&format!("/{}/", middle));
        }
    }

    if parts.len() == 2 {
        if parts[0].is_empty() {
            // Pattern like "**/suffix" — match suffix where * acts as wildcard
            let suffix = parts[1].trim_start_matches('/');
            let path_segments: Vec<&str> = path.split('/').collect();
            let suffix_segments: Vec<&str> = suffix.split('/').collect();
            if suffix_segments.len() > path_segments.len() {
                return false;
            }
            for start in 0..=(path_segments.len() - suffix_segments.len()) {
                let all_match = suffix_segments
                    .iter()
                    .zip(&path_segments[start..])
                    .all(|(ps, ts)| segment_matches(ps, ts));
                if all_match {
                    return true;
                }
            }
            false
        } else if parts[1].is_empty() {
            // Pattern like "prefix/**" — prefix match
            path.starts_with(parts[0].trim_end_matches('/'))
        } else {
            false
        }
    } else if pattern.contains('*') {
        path_matches_simple(pattern, path)
    } else {
        false
    }
}

fn path_matches_simple(pattern: &str, path: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();
    if pattern_parts.len() != path_parts.len() {
        return false;
    }
    pattern_parts
        .iter()
        .zip(path_parts.iter())
        .all(|(p, t)| segment_matches(p, t))
}

fn segment_matches(pattern: &str, segment: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == segment;
    }
    // Simple prefix*/suffix matching
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 2 {
        if parts[0].is_empty() {
            segment.ends_with(parts[1])
        } else if parts[1].is_empty() {
            segment.starts_with(parts[0])
        } else {
            segment.starts_with(parts[0]) && segment.ends_with(parts[1])
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_sandbox_bash_disabled() {
        let sandbox = ExecutionSandbox::default();
        assert!(!sandbox.bash_enabled);
        assert!(sandbox.validate("ls", None).is_err());
        assert!(sandbox
            .validate("ls", None)
            .unwrap_err()
            .contains("disabled"));
    }

    #[test]
    fn test_dangerous_commands_blocked() {
        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;

        for cmd in &["rm -rf /", "sudo ls", "kill 1234", "curl evil.com"] {
            assert!(
                sandbox.is_command_dangerous(cmd),
                "command '{}' should be dangerous",
                cmd
            );
            assert!(
                sandbox.validate(cmd, None).is_err(),
                "command '{}' should be blocked",
                cmd
            );
        }
    }

    #[test]
    fn test_allowed_commands_pass_validation() {
        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;

        for cmd in &["ls", "cat file.txt", "echo hello", "pwd", "date"] {
            assert!(
                sandbox.is_command_allowed(cmd),
                "command '{}' should be allowed",
                cmd
            );
            assert!(
                sandbox.validate(cmd, None).is_ok(),
                "command '{}' should pass",
                cmd
            );
        }
    }

    #[test]
    fn test_deny_read_blocks_secret_paths() {
        let sandbox = ExecutionSandbox::default();

        let secret_paths = [
            "/home/user/.env",
            "/app/.env.production",
            "/root/.ssh/id_rsa",
            "/root/.ssh/id_ed25519",
            "/etc/ssl/private/key.pem",
            "/home/user/.aws/credentials",
            "/app/config/secrets/db.json",
            "/home/user/.git-credentials",
        ];

        for path in &secret_paths {
            assert!(
                sandbox.is_path_denied_read(path),
                "path '{}' should be denied read",
                path
            );
        }
    }

    #[test]
    fn test_safe_paths_allow_expected_reads() {
        let sandbox = ExecutionSandbox::default();

        let safe = ["/tmp/output.txt", "./result.log"];
        for path in &safe {
            assert!(sandbox.is_path_in_safe_paths(path));
            assert!(!sandbox.is_path_denied_read(path));
        }
    }

    #[test]
    fn test_command_not_in_allowlist_blocked() {
        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;

        let result = sandbox.validate("python script.py", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the allowed command"));
    }

    #[test]
    fn test_env_allowlist() {
        let sandbox = ExecutionSandbox::default();

        assert!(sandbox.is_env_allowed("PATH"));
        assert!(sandbox.is_env_allowed("HOME"));
        assert!(sandbox.is_env_allowed("USER"));
        assert!(!sandbox.is_env_allowed("SECRET_KEY"));
        assert!(!sandbox.is_env_allowed("AWS_ACCESS_KEY_ID"));
    }

    #[test]
    fn test_denied_cwd_blocks_execution() {
        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;

        let result = sandbox.validate("ls", Some("/root/.ssh"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("denied"));
    }

    #[test]
    fn test_sandbox_serialization() {
        let sandbox = ExecutionSandbox::default();
        let json = serde_json::to_string(&sandbox).unwrap();
            assert!(json.contains("bashEnabled"));
        assert!(json.contains("denyReadPatterns"));
        assert!(json.contains("dangerousCommandDenylist"));

        let deserialized: ExecutionSandbox = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.bash_enabled, false);
        assert_eq!(deserialized.network_policy, NetworkPolicy::None);
        assert_eq!(deserialized.write_policy, WritePolicy::ProposalFirst);
    }

    #[test]
    fn test_sandbox_with_write_disabled_cannot_write_to_denied_paths() {
        let mut sandbox = ExecutionSandbox::default();
        sandbox.write_policy = WritePolicy::SafePathsOnly;

        assert!(sandbox.is_path_denied_write("/etc/hosts"));
        assert!(sandbox.is_path_denied_write("/System/Library/test"));
        assert!(!sandbox.is_path_denied_write("/tmp/safe_output.txt"));
    }
}
