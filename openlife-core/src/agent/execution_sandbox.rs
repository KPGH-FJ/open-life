//! ExecutionSandbox — policy types and validation helpers for shell execution.
//!
//! No shell execution happens here. This module defines the sandbox policy
//! primitives that any future bash/shell executor must enforce.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

/// Kind of filesystem access for operand validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathAccessKind {
    Read,
    Write,
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
    // ── Path validation (canonicalize-based) ──────────────────────────

    /// Canonicalize a list of path strings. Paths that cannot be resolved
    /// (nonexistent) are dropped with a warning.
    fn canonicalize_safe_paths(&self) -> Vec<PathBuf> {
        self.safe_paths
            .iter()
            .filter_map(|p| {
                std::fs::canonicalize(p).ok().or_else(|| {
                    // If the path itself doesn't exist, try its parent
                    // so symlinks in the existing portion are resolved.
                    let path = Path::new(p);
                    path.parent().and_then(|parent| {
                        std::fs::canonicalize(parent).ok().map(|canon_parent| {
                            canon_parent.join(path.file_name().unwrap_or_default())
                        })
                    })
                })
            })
            .collect()
    }

    /// Try to canonicalize a path that must already exist.
    /// Returns `Err(reason)` if the path doesn't exist on disk.
    fn try_canonicalize_existing(path: &str) -> Result<PathBuf, String> {
        std::fs::canonicalize(path)
            .map_err(|_| format!("path '{}' does not exist or cannot be resolved", path))
    }

    /// Try to canonicalize a path. Falls back to parent canonicalization
    /// for paths that do not yet exist (e.g., planned output files).
    fn try_canonicalize(path: &str) -> Result<PathBuf, String> {
        if let Ok(canon) = std::fs::canonicalize(path) {
            return Ok(canon);
        }
        // Path may not exist yet — canonicalize parent, then append filename.
        let p = Path::new(path);
        match p.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                let canon_parent = std::fs::canonicalize(parent).map_err(|_| {
                    format!(
                        "path '{}' does not exist and parent '{}' cannot be resolved",
                        path,
                        parent.display()
                    )
                })?;
                Ok(canon_parent.join(p.file_name().unwrap_or_default()))
            }
            _ => Err(format!("cannot canonicalize path '{}'", path)),
        }
    }

    /// Validate that a path is in safe_paths, checking deny patterns first.
    /// Uses canonicalization so symlinks and parent dirs are resolved before
    /// comparing prefixes.
    ///
    /// Returns `Ok(())` if the path is allowed, `Err(reason)` if blocked.
    pub fn validate_path_in_sandbox(
        &self,
        path: &str,
        access_kind: PathAccessKind,
    ) -> Result<(), String> {
        // 1. Check deny patterns first (takes priority)
        match access_kind {
            PathAccessKind::Read => {
                if self.is_path_denied_read(path) {
                    return Err(format!(
                        "path '{}' is denied (matches read deny pattern)",
                        path
                    ));
                }
            }
            PathAccessKind::Write => {
                if self.is_path_denied_write(path) {
                    return Err(format!(
                        "path '{}' is denied (matches write deny pattern)",
                        path
                    ));
                }
            }
        }

        // 2. Canonicalize and check against canonicalized safe_paths
        let canon_path =
            Self::try_canonicalize(path).map_err(|e| format!("path validation failed: {}", e))?;
        let canon_safe = self.canonicalize_safe_paths();

        let in_safe = canon_safe
            .iter()
            .any(|safe| canon_path.starts_with(safe) || canon_path == *safe);

        if !in_safe {
            return Err(format!(
                "path '{}' (resolved to '{}') is not within any safe path",
                path,
                canon_path.display()
            ));
        }

        Ok(())
    }

    /// Validate a path operand from a command. This is the helper intended
    /// for future bash/shell executors to validate file arguments (e.g.,
    /// `cat /some/path` or `echo "text" > /some/path`).
    ///
    /// Uses the same canonicalize-based logic as `validate_path_in_sandbox`.
    pub fn validate_path_operand(
        &self,
        operand: &str,
        access_kind: PathAccessKind,
    ) -> Result<(), String> {
        // Strip shell operators if present (">>", ">", "<")
        let clean = operand.trim().trim_start_matches(['>', '<', ' ']);
        if clean.is_empty() {
            return Ok(()); // empty operands are not paths
        }
        self.validate_path_in_sandbox(clean, access_kind)
    }

    // ── Legacy path helpers (non-canonicalize, for deny-only checks) ──

    /// Check whether a path is within the allowed safe_paths.
    /// Prefer `validate_path_in_sandbox` for security-critical checks.
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
        self.command_allowlist.iter().any(|c| c == cmd_name)
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

    /// Validate a command and cwd against the sandbox policy.
    ///
    /// Checks (in order):
    /// 1. Bash must be enabled.
    /// 2. Command must not be on the dangerous denylist.
    /// 3. Command must be in the allowlist (if non-empty).
    /// 4. cwd must not match deny_read or deny_write patterns.
    /// 5. cwd must be within a canonicalized safe_path.
    ///
    /// Returns `Ok(())` if allowed, `Err(reason)` if blocked.
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
            // Deny patterns checked first (takes priority over safe paths)
            if self.is_path_denied_read(dir) {
                return Err(format!(
                    "working directory '{}' is denied (read block)",
                    dir
                ));
            }
            if self.is_path_denied_write(dir) {
                return Err(format!(
                    "working directory '{}' is denied (write block)",
                    dir
                ));
            }

            // Canonicalize cwd and verify it's in safe_paths.
            // cwd MUST exist — use strict canonicalize.
            let canon_cwd = Self::try_canonicalize_existing(dir)
                .map_err(|e| format!("working directory '{}' cannot be resolved: {}", dir, e))?;
            let canon_safe = self.canonicalize_safe_paths();
            let in_safe = canon_safe.iter().any(|safe| canon_cwd.starts_with(safe));
            if !in_safe {
                return Err(format!(
                    "working directory '{}' (resolved to '{}') is not within any safe path",
                    dir,
                    canon_cwd.display()
                ));
            }
        }

        Ok(())
    }
}

// ── Glob matching ──────────────────────────────────────────────────────

const PATH_SEP: char = std::path::MAIN_SEPARATOR;

/// Simple glob matching. Supports `**` and `*` wildcards.
fn glob_matches(pattern: &str, path: &str) -> bool {
    if pattern == path {
        return true;
    }
    // Handle absolute prefix patterns
    if pattern.starts_with(PATH_SEP) && !path.starts_with(PATH_SEP) {
        // Pattern is absolute, path is relative — no match
        if !pattern.contains("**") {
            return false;
        }
    }
    // Handle **/middle/** patterns (three segments)
    let parts: Vec<&str> = pattern.split("**").collect();
    if parts.len() == 3 && parts[0].is_empty() && parts[2].is_empty() {
        let middle = parts[1].trim_matches(PATH_SEP);
        if middle.contains('*') {
            // Fall through to segment-based matching
        } else {
            return path.split(PATH_SEP).any(|seg| seg == middle)
                || path.contains(&format!("{}{}{}", PATH_SEP, middle, PATH_SEP))
                || path.ends_with(&format!("{}{}", PATH_SEP, middle));
        }
    }

    if parts.len() == 2 {
        if parts[0].is_empty() {
            // Pattern like "**/suffix" — match suffix where * acts as wildcard
            let suffix = parts[1].trim_start_matches(PATH_SEP);
            let path_segments: Vec<&str> = path.split(PATH_SEP).collect();
            let suffix_segments: Vec<&str> = suffix.split(PATH_SEP).collect();
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
            path.starts_with(parts[0].trim_end_matches(PATH_SEP))
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
    let pattern_parts: Vec<&str> = pattern.split(PATH_SEP).collect();
    let path_parts: Vec<&str> = path.split(PATH_SEP).collect();
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

    fn make_enabled_sandbox() -> ExecutionSandbox {
        let mut s = ExecutionSandbox::default();
        s.bash_enabled = true;
        s
    }

    // ── Bash disabled ──────────────────────────────────────────────────

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

    // ── Command filtering ──────────────────────────────────────────────

    #[test]
    fn test_dangerous_commands_blocked() {
        let sandbox = make_enabled_sandbox();
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
        let sandbox = make_enabled_sandbox();
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
    fn test_command_not_in_allowlist_blocked() {
        let sandbox = make_enabled_sandbox();
        let result = sandbox.validate("python script.py", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the allowed command"));
    }

    // ── Env allowlist ──────────────────────────────────────────────────

    #[test]
    fn test_env_allowlist() {
        let sandbox = ExecutionSandbox::default();
        assert!(sandbox.is_env_allowed("PATH"));
        assert!(sandbox.is_env_allowed("HOME"));
        assert!(sandbox.is_env_allowed("USER"));
        assert!(!sandbox.is_env_allowed("SECRET_KEY"));
        assert!(!sandbox.is_env_allowed("AWS_ACCESS_KEY_ID"));
    }

    // ── Deny read patterns ─────────────────────────────────────────────

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

    // ── Legacy safe_path helper ────────────────────────────────────────

    #[test]
    fn test_safe_paths_allow_expected_reads() {
        let sandbox = ExecutionSandbox::default();
        let safe = ["/tmp/output.txt", "./result.log"];
        for path in &safe {
            assert!(sandbox.is_path_in_safe_paths(path));
            assert!(!sandbox.is_path_denied_read(path));
        }
    }

    // ── Path canonicalization hardening ────────────────────────────────

    #[test]
    fn test_safe_path_subdir_allowed() {
        let sandbox = ExecutionSandbox {
            safe_paths: vec!["/tmp".into()],
            ..ExecutionSandbox::default()
        };
        // /tmp_allowed is a sibling, NOT a subdir — should be rejected
        assert!(
            sandbox
                .validate_path_in_sandbox("/tmp_allowed/foo", PathAccessKind::Read)
                .is_err(),
            "/tmp_allowed should NOT match safe_path /tmp"
        );
        // /tmp/subdir is a real subdirectory — should be allowed
        let result = sandbox.validate_path_in_sandbox("/tmp", PathAccessKind::Read);
        assert!(
            result.is_ok(),
            "/tmp itself should be allowed: {:?}",
            result
        );
    }

    #[test]
    fn test_tmp_evil_not_allowed_by_startswith() {
        let sandbox = ExecutionSandbox {
            safe_paths: vec!["/tmp".into()],
            ..ExecutionSandbox::default()
        };
        // /tmp_evil starts with "/tmp" as a string but is NOT under /tmp/
        let result = sandbox.validate_path_in_sandbox("/tmp_evil/script.sh", PathAccessKind::Read);
        assert!(
            result.is_err(),
            "/tmp_evil should NOT be allowed by starts_with('/tmp'): {:?}",
            result
        );
    }

    #[test]
    fn test_relative_path_dot_dot_blocked() {
        let sandbox = ExecutionSandbox {
            safe_paths: vec!["/tmp".into()],
            ..ExecutionSandbox::default()
        };
        // ../secret relative to /tmp would escape safe_paths
        let result = sandbox.validate_path_in_sandbox("/tmp/../secret", PathAccessKind::Read);
        // canonicalize resolves /tmp/../secret to /secret which is not in /tmp
        assert!(
            result.is_err(),
            "/tmp/../secret should resolve outside /tmp: {:?}",
            result
        );
    }

    #[test]
    fn test_deny_pattern_priority_over_safe_path() {
        // Even if a path is in safe_paths, deny patterns take priority
        let mut sandbox = ExecutionSandbox {
            safe_paths: vec!["/tmp".into()],
            ..ExecutionSandbox::default()
        };
        sandbox.deny_read_patterns = vec!["**/.env".into()];
        sandbox.deny_write_patterns = vec!["**/.env".into()];

        // /tmp/.env is in safe_paths but ALSO matches deny pattern — should be denied
        let result = sandbox.validate_path_in_sandbox("/tmp/.env", PathAccessKind::Read);
        assert!(
            result.is_err(),
            "/tmp/.env should be denied even though it's in safe_paths: {:?}",
            result
        );
        assert!(result.unwrap_err().contains("deny pattern"));
    }

    #[test]
    fn test_nonexistent_cwd_rejected() {
        let sandbox = make_enabled_sandbox();
        let result = sandbox.validate("ls", Some("/tmp/nonexistent_dir_12345"));
        assert!(
            result.is_err(),
            "nonexistent cwd should be rejected: {:?}",
            result
        );
    }

    // ── Path operand validation ────────────────────────────────────────

    #[test]
    fn test_validate_path_operand_allowed() {
        let sandbox = ExecutionSandbox {
            safe_paths: vec!["/tmp".into()],
            ..ExecutionSandbox::default()
        };
        // cat /tmp/file → path operand /tmp/file is allowed
        let result = sandbox.validate_path_operand("/tmp/file.txt", PathAccessKind::Read);
        assert!(result.is_ok(), "expected ok: {:?}", result);
    }

    #[test]
    fn test_validate_path_operand_denied() {
        let sandbox = ExecutionSandbox {
            safe_paths: vec!["/tmp".into()],
            ..ExecutionSandbox::default()
        };
        // cat /etc/passwd → /etc is not in safe_paths
        let result = sandbox.validate_path_operand("/etc/passwd", PathAccessKind::Read);
        assert!(
            result.is_err(),
            "/etc/passwd should be denied as operand: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_path_operand_strips_redirect() {
        let sandbox = ExecutionSandbox {
            safe_paths: vec!["/tmp".into()],
            ..ExecutionSandbox::default()
        };
        // echo "text" >> /tmp/log → operand with >> prefix should be cleaned
        let result = sandbox.validate_path_operand(">> /tmp/log", PathAccessKind::Write);
        assert!(
            result.is_ok(),
            "expected ok after stripping >>: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_path_operand_empty_passes() {
        let sandbox = ExecutionSandbox::default();
        // Empty operands (like >> or redirect with no path) are not paths
        assert!(sandbox
            .validate_path_operand("", PathAccessKind::Read)
            .is_ok());
        assert!(sandbox
            .validate_path_operand("   ", PathAccessKind::Write)
            .is_ok());
    }

    // ── cwd validation ─────────────────────────────────────────────────

    #[test]
    fn test_denied_cwd_blocks_execution() {
        let sandbox = make_enabled_sandbox();
        let result = sandbox.validate("ls", Some("/root/.ssh"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("denied"));
    }

    #[test]
    fn test_cwd_not_in_safe_paths_rejected() {
        let sandbox = ExecutionSandbox {
            safe_paths: vec!["/tmp".into()],
            bash_enabled: true,
            ..ExecutionSandbox::default()
        };
        // /usr is not in safe_paths
        let result = sandbox.validate("ls", Some("/usr"));
        assert!(
            result.is_err(),
            "cwd /usr not in safe_paths should be rejected: {:?}",
            result
        );
    }

    // ── Serialization ──────────────────────────────────────────────────

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
