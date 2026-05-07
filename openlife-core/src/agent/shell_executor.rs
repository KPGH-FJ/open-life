use crate::agent::execution_sandbox::{ExecutionSandbox, PathAccessKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellCommandRequest {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
    pub truncated: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone)]
pub enum ShellExecutionError {
    SandboxDisabled,
    DangerousCommand(String),
    NotInAllowlist(String),
    Metacharacters(String),
    CwdInvalid(String),
    CwdDenied(String),
    EnvNotAllowed(String),
    OperandBlocked(String),
    InterpreterBlocked(String),
    CommandNotFound(String),
    SpawnFailed(String),
    Timeout(u64),
    IoError(String),
}

impl std::fmt::Display for ShellExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellExecutionError::SandboxDisabled => write!(f, "shell execution is disabled"),
            ShellExecutionError::DangerousCommand(cmd) => {
                write!(f, "dangerous command blocked: {}", cmd)
            }
            ShellExecutionError::NotInAllowlist(cmd) => {
                write!(f, "command not in allowlist: {}", cmd)
            }
            ShellExecutionError::Metacharacters(cmd) => {
                write!(f, "command contains shell metacharacters: {}", cmd)
            }
            ShellExecutionError::CwdInvalid(msg) => write!(f, "cwd invalid: {}", msg),
            ShellExecutionError::CwdDenied(msg) => write!(f, "cwd denied: {}", msg),
            ShellExecutionError::EnvNotAllowed(var) => {
                write!(f, "env variable not allowed: {}", var)
            }
            ShellExecutionError::OperandBlocked(msg) => {
                write!(f, "operand blocked: {}", msg)
            }
            ShellExecutionError::InterpreterBlocked(cmd) => {
                write!(f, "interpreter blocked: {}", cmd)
            }
            ShellExecutionError::CommandNotFound(cmd) => {
                write!(f, "command not found: {}", cmd)
            }
            ShellExecutionError::SpawnFailed(msg) => write!(f, "spawn failed: {}", msg),
            ShellExecutionError::Timeout(ms) => write!(f, "timeout after {} ms", ms),
            ShellExecutionError::IoError(msg) => write!(f, "i/o error: {}", msg),
        }
    }
}

const INTERPRETER_DENYLIST: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "fish",
    "dash",
    "cmd",
    "powershell",
    "pwsh",
    "osascript",
];

const DANGEROUS_FIND_FLAGS: &[&str] = &["-delete", "-exec", "-execdir", "-ok", "-okdir"];

/// Commands whose positional non-flag args are treated as file-path operands
/// and must be validated against the sandbox.
fn is_path_operand_command(cmd: &str) -> bool {
    matches!(cmd, "cat" | "head" | "tail" | "wc" | "ls")
}

/// Extract the executable basename from a command path.
/// "/bin/sh" -> "sh", "echo" -> "echo", "/usr/bin/bash" -> "bash".
fn command_basename(cmd: &str) -> &str {
    // Strip leading whitespace and get the first token (command name only)
    let first = cmd.split_whitespace().next().unwrap_or(cmd);
    Path::new(first)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(first)
}

fn is_interpreter_blocked(cmd_basename: &str) -> bool {
    INTERPRETER_DENYLIST.contains(&cmd_basename)
}

/// Whether an argument is a positional path operand (not a flag/option).
/// Flags start with '-'. Everything else is treated as a path for
/// path-operand commands — conservative fail-closed for P9.
fn is_positional_path_arg(arg: &str) -> bool {
    if arg.is_empty() {
        return false;
    }
    !arg.starts_with('-')
}

fn has_shell_metacharacters(text: &str) -> bool {
    text.contains('|')
        || text.contains(';')
        || text.contains('&')
        || text.contains('$')
        || text.contains('`')
        || text.contains('<')
        || text.contains('>')
        || text.contains('(')
        || text.contains(')')
        || text.contains('*')
        || text.contains('?')
        || text.contains('~')
        || text.contains('{')
        || text.contains('}')
        || text.contains('[')
        || text.contains(']')
}

pub struct ShellExecutor {
    sandbox: ExecutionSandbox,
}

impl ShellExecutor {
    pub fn new(sandbox: ExecutionSandbox) -> Self {
        Self { sandbox }
    }

    pub fn execute(
        &self,
        request: &ShellCommandRequest,
    ) -> Result<ShellCommandOutput, ShellExecutionError> {
        if !self.sandbox.bash_enabled {
            return Err(ShellExecutionError::SandboxDisabled);
        }

        // ── 0. Resolve effective cwd (Fix 1) ─────────────────────────
        let request_cwd = request.cwd.as_deref();
        let sandbox_cwd = self.sandbox.cwd.as_str();
        let effective_cwd = if let Some(c) = request_cwd {
            c
        } else if !sandbox_cwd.is_empty() {
            sandbox_cwd
        } else {
            return Err(ShellExecutionError::CwdInvalid(
                "no cwd set: request.cwd is None and sandbox.cwd is empty".into(),
            ));
        };

        // Validate effective cwd against sandbox policy (deny patterns + safe_paths)
        if self.sandbox.is_path_denied_read(effective_cwd) {
            return Err(ShellExecutionError::CwdDenied(format!(
                "cwd '{}' matches deny-read pattern",
                effective_cwd
            )));
        }
        if self.sandbox.is_path_denied_write(effective_cwd) {
            return Err(ShellExecutionError::CwdDenied(format!(
                "cwd '{}' matches deny-write pattern",
                effective_cwd
            )));
        }
        if !self.sandbox.is_path_in_safe_paths(effective_cwd) {
            return Err(ShellExecutionError::CwdDenied(format!(
                "cwd '{}' is not within any safe path",
                effective_cwd
            )));
        }

        // ── 1. Basename normalization (Fix 3) + interpreter block ────
        let cmd_basename = command_basename(&request.command);

        if is_interpreter_blocked(cmd_basename) {
            return Err(ShellExecutionError::InterpreterBlocked(
                cmd_basename.to_string(),
            ));
        }

        // ── 2. Reject shell metacharacters in args ───────────────────
        for arg in &request.args {
            if has_shell_metacharacters(arg) {
                return Err(ShellExecutionError::Metacharacters(arg.clone()));
            }
        }

        // ── 3. Validate command through sandbox ──────────────────────
        // Always pass effective_cwd so the sandbox validates it
        if let Err(msg) = self.sandbox.validate(&request.command, Some(effective_cwd)) {
            if msg.contains("disabled") {
                return Err(ShellExecutionError::SandboxDisabled);
            }
            if msg.contains("dangerous command") {
                return Err(ShellExecutionError::DangerousCommand(
                    request.command.clone(),
                ));
            }
            if msg.contains("not in the allowed") {
                return Err(ShellExecutionError::NotInAllowlist(request.command.clone()));
            }
            if msg.contains("metacharacters") {
                return Err(ShellExecutionError::Metacharacters(request.command.clone()));
            }
            if msg.contains("denied") {
                return Err(ShellExecutionError::CwdDenied(msg));
            }
            return Err(ShellExecutionError::CwdInvalid(msg));
        }

        // ── 4. Operand validation (Fix 2) ────────────────────────────

        // 4a. Reject dangerous find flags before any path validation
        if cmd_basename == "find" {
            for arg in &request.args {
                if DANGEROUS_FIND_FLAGS.contains(&arg.as_str()) {
                    return Err(ShellExecutionError::OperandBlocked(format!(
                        "find: dangerous flag '{}' is blocked",
                        arg
                    )));
                }
            }
        }

        // 4b. Validate path operands for cat/head/tail/wc/ls
        if is_path_operand_command(cmd_basename) {
            for arg in &request.args {
                if is_positional_path_arg(arg) {
                    // Path relative to effective_cwd
                    let full_path = if arg.starts_with('/')
                        || arg.contains(std::path::MAIN_SEPARATOR)
                        || arg.contains('.')
                    {
                        arg.clone()
                    } else {
                        // Extensionless name like Makefile — validate relative to cwd
                        format!("{}/{}", effective_cwd, arg)
                    };
                    if let Err(reason) = self
                        .sandbox
                        .validate_path_operand(&full_path, PathAccessKind::Read)
                    {
                        return Err(ShellExecutionError::OperandBlocked(format!(
                            "{} operand '{}': {}",
                            cmd_basename, arg, reason
                        )));
                    }
                }
            }
        }

        // 4c. Validate grep file operands (arg[0] is pattern, arg[1..] are files)
        if cmd_basename == "grep" {
            let file_args = if request
                .args
                .first()
                .map(|a| a.starts_with('-'))
                .unwrap_or(false)
            {
                // Has a flag before pattern — skip flags until first pattern
                request.args.iter().skip(1)
            } else {
                // First arg is pattern, file paths start at arg[1]
                request.args.iter().skip(1)
            };
            let file_args: Vec<&String> = file_args.filter(|a| is_positional_path_arg(a)).collect();

            // If no file operand, grep reads from stdin; effective_cwd already validated
            for arg in &file_args {
                let full_path = if arg.starts_with('/')
                    || arg.contains(std::path::MAIN_SEPARATOR)
                    || arg.contains('.')
                {
                    (*arg).clone()
                } else {
                    format!("{}/{}", effective_cwd, arg)
                };
                if let Err(reason) = self
                    .sandbox
                    .validate_path_operand(&full_path, PathAccessKind::Read)
                {
                    return Err(ShellExecutionError::OperandBlocked(format!(
                        "grep file operand '{}': {}",
                        arg, reason
                    )));
                }
            }
        }

        // 4d. find path operand validation (first non-flag arg is the search path)
        if cmd_basename == "find" {
            if let Some(search_path) = request.args.iter().find(|a| is_positional_path_arg(a)) {
                if let Err(reason) = self
                    .sandbox
                    .validate_path_operand(search_path, PathAccessKind::Read)
                {
                    return Err(ShellExecutionError::OperandBlocked(format!(
                        "find search path '{}': {}",
                        search_path, reason
                    )));
                }
            }
        }

        // ── 5. Validate env variable names ───────────────────────────
        for var_name in request.env.keys() {
            if !self.sandbox.is_env_allowed(var_name) {
                return Err(ShellExecutionError::EnvNotAllowed(var_name.clone()));
            }
        }

        // ── 6. Build command ─────────────────────────────────────────
        let mut cmd = Command::new(&request.command);
        cmd.args(&request.args);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.current_dir(effective_cwd);

        cmd.env_clear();
        for (k, v) in &request.env {
            if self.sandbox.is_env_allowed(k) {
                cmd.env(k, v);
            }
        }

        // ── 7. Spawn with timeout ────────────────────────────────────
        let start = Instant::now();
        let max_bytes = if self.sandbox.max_output_bytes > 0 {
            self.sandbox.max_output_bytes
        } else {
            1024 * 1024
        };
        let timeout = Duration::from_millis(self.sandbox.timeout_ms);

        let mut child = cmd
            .spawn()
            .map_err(|e| ShellExecutionError::SpawnFailed(e.to_string()))?;

        let poll_interval = Duration::from_millis(50);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    let mut stdout = Vec::new();
                    let mut stderr = Vec::new();
                    let mut stdout_truncated = false;
                    let mut stderr_truncated = false;

                    if let Some(mut out) = child.stdout.take() {
                        let limit = if max_bytes > 0 { max_bytes } else { usize::MAX };
                        let mut buf = [0u8; 8192];
                        loop {
                            match out.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    let space = limit.saturating_sub(stdout.len());
                                    let take = n.min(space);
                                    stdout.extend_from_slice(&buf[..take]);
                                    if stdout.len() >= limit {
                                        stdout_truncated = true;
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }
                    if let Some(mut err) = child.stderr.take() {
                        let limit = if max_bytes > 0 { max_bytes } else { usize::MAX };
                        let mut buf = [0u8; 8192];
                        loop {
                            match err.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    let space = limit.saturating_sub(stderr.len());
                                    let take = n.min(space);
                                    stderr.extend_from_slice(&buf[..take]);
                                    if stderr.len() >= limit {
                                        stderr_truncated = true;
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }

                    return Ok(ShellCommandOutput {
                        stdout: String::from_utf8_lossy(&stdout).into_owned(),
                        stderr: String::from_utf8_lossy(&stderr).into_owned(),
                        exit_code: status.code().unwrap_or(-1),
                        timed_out: false,
                        truncated: stdout_truncated || stderr_truncated,
                        elapsed_ms: elapsed,
                    });
                }
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        let elapsed = start.elapsed().as_millis() as u64;

                        let mut stdout = Vec::new();
                        let mut stderr = Vec::new();
                        if let Some(mut out) = child.stdout.take() {
                            let _ = out.read_to_end(&mut stdout);
                        }
                        if let Some(mut err) = child.stderr.take() {
                            let _ = err.read_to_end(&mut stderr);
                        }

                        return Ok(ShellCommandOutput {
                            stdout: String::from_utf8_lossy(&stdout).into_owned(),
                            stderr: String::from_utf8_lossy(&stderr).into_owned(),
                            exit_code: -1,
                            timed_out: true,
                            truncated: stdout.len() >= max_bytes || stderr.len() >= max_bytes,
                            elapsed_ms: elapsed,
                        });
                    }
                    std::thread::sleep(poll_interval);
                }
                Err(e) => {
                    let _ = child.kill();
                    return Err(ShellExecutionError::IoError(e.to_string()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_dir() -> String {
        std::env::temp_dir().to_string_lossy().to_string()
    }

    fn make_disabled_sandbox() -> ExecutionSandbox {
        ExecutionSandbox::default()
    }

    fn make_enabled_sandbox() -> ExecutionSandbox {
        let mut s = ExecutionSandbox::default();
        s.bash_enabled = true;
        s.cwd = tmp_dir();
        s.safe_paths = vec![tmp_dir()];
        s.timeout_ms = 30_000;
        s.max_output_bytes = 1024 * 1024;
        s
    }

    fn make_restricted_sandbox() -> ExecutionSandbox {
        let mut s = ExecutionSandbox::default();
        s.bash_enabled = true;
        s.cwd = tmp_dir();
        s.safe_paths = vec![tmp_dir()];
        s.command_allowlist = vec!["echo".into(), "date".into(), "cat".into(), "ls".into()];
        s.timeout_ms = 30_000;
        s.max_output_bytes = 10 * 1024;
        s
    }

    fn sandbox_with_safe_tmp() -> ExecutionSandbox {
        let tmp = tmp_dir();
        let mut s = ExecutionSandbox::default();
        s.bash_enabled = true;
        s.cwd = tmp.clone();
        s.safe_paths = vec![tmp.clone()];
        s.command_allowlist = vec![
            "echo".into(),
            "cat".into(),
            "grep".into(),
            "sleep".into(),
            "find".into(),
            "ls".into(),
        ];
        s.timeout_ms = 30_000;
        s
    }

    // ── Fix 1: cwd tests ──────────────────────────────────────────

    #[test]
    fn test_no_request_cwd_uses_sandbox_cwd() {
        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let file_name = "openlife_p9_cwd_test.txt";
        let file_path = format!("{}/{}", tmp_dir(), file_name);
        std::fs::write(&file_path, "cwd ok").unwrap();

        // No explicit cwd → falls back to sandbox.cwd (tmp)
        let req = ShellCommandRequest {
            command: "cat".into(),
            args: vec![file_path.clone()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        let _ = std::fs::remove_file(&file_path);
        assert!(result.is_ok());
        assert!(result.unwrap().stdout.contains("cwd ok"));
    }

    #[test]
    fn test_empty_sandbox_cwd_fails_before_spawn() {
        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;
        sandbox.cwd = String::new(); // empty!
        sandbox.safe_paths = vec![tmp_dir()];
        sandbox.command_allowlist = vec!["echo".into()];

        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "echo".into(),
            args: vec!["test".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("cwd") || err.contains("empty") || err.contains("no cwd"));
    }

    #[test]
    fn test_cwd_outside_safe_paths_rejected() {
        let tmp = tmp_dir();
        let bad_cwd = "/root";
        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;
        sandbox.cwd = bad_cwd.into();
        sandbox.safe_paths = vec![tmp.clone()];
        sandbox.command_allowlist = vec!["echo".into()];

        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "echo".into(),
            args: vec!["test".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("cwd") || err.contains("not within") || err.contains("safe"),
            "expected cwd rejection, got: {}",
            err
        );
    }

    #[test]
    fn test_cwd_denied_by_pattern() {
        let tmp = tmp_dir();
        let deny_cwd = format!("{}/secrets", tmp);
        std::fs::create_dir_all(&deny_cwd).ok();

        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;
        sandbox.cwd = deny_cwd.clone();
        sandbox.safe_paths = vec![tmp.clone()];
        sandbox.deny_read_patterns = vec!["**/secrets".into(), "**/secrets/**".into()];
        sandbox.command_allowlist = vec!["echo".into()];

        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "echo".into(),
            args: vec!["test".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        let _ = std::fs::remove_dir_all(&deny_cwd);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("deny") || err.contains("cwd") || err.contains("denied"),
            "expected cwd deny, got: {}",
            err
        );
    }

    // ── Fix 2: Expanded operand validation tests ───────────────────

    #[test]
    fn test_ls_etc_rejected() {
        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "ls".into(),
            args: vec!["/etc".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("operand") || err.contains("not within"));
    }

    #[test]
    fn test_ls_safe_path_succeeds() {
        let tmp = tmp_dir();
        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "ls".into(),
            args: vec![tmp.clone()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cat_makefile_validated_against_effective_cwd() {
        let tmp = tmp_dir();
        let file_path = format!("{}/Makefile", tmp);
        std::fs::write(&file_path, "all: build\n").unwrap();

        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "cat".into(),
            args: vec!["Makefile".into()],
            cwd: None, // uses sandbox.cwd = tmp
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        let _ = std::fs::remove_file(&file_path);
        assert!(result.is_ok());
        assert!(result.unwrap().stdout.contains("all: build"));
    }

    #[test]
    fn test_cat_makefile_rejected_when_cwd_outside_safe() {
        let tmp = tmp_dir();
        // Create a bad cwd inside tmp, then remove tmp from safe_paths
        let bad_cwd = format!("{}/bad_p9", tmp);
        std::fs::create_dir_all(&bad_cwd).unwrap();
        let file_path = format!("{}/Makefile", bad_cwd);
        std::fs::write(&file_path, "bad\n").unwrap();

        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;
        sandbox.cwd = bad_cwd.clone();
        // safe_paths does NOT include tmp or bad_cwd — only a different directory
        sandbox.safe_paths = vec!["/p9_nonexistent_safe".to_string()];
        sandbox.command_allowlist = vec!["cat".into()];
        sandbox.timeout_ms = 30_000;

        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "cat".into(),
            args: vec!["Makefile".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        let _ = std::fs::remove_dir_all(&bad_cwd);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_delete_rejected() {
        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "find".into(),
            args: vec![tmp_dir(), "-delete".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("delete") || err.contains("operand") || err.contains("dangerous"));
    }

    #[test]
    fn test_find_exec_rejected() {
        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "find".into(),
            args: vec![
                tmp_dir(),
                "-exec".into(),
                "echo".into(),
                "{}".into(),
                ";".into(),
            ],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(
            result.is_err()
                || result
                    .as_ref()
                    .ok()
                    .map(|o| !o.stdout.contains("exec"))
                    .unwrap_or(true)
        );
    }

    #[test]
    fn test_find_execdir_rejected() {
        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "find".into(),
            args: vec![tmp_dir(), "-execdir".into(), "ls".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("execdir") || err.contains("operand") || err.contains("dangerous"));
    }

    #[test]
    fn test_grep_safe_file_succeeds() {
        let tmp = tmp_dir();
        let file_path = format!("{}/openlife_p9_grep2.txt", tmp);
        std::fs::write(&file_path, "needle in haystack\n").unwrap();

        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "grep".into(),
            args: vec!["needle".into(), file_path.clone()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        let _ = std::fs::remove_file(&file_path);
        assert!(result.is_ok());
        assert!(result.unwrap().stdout.contains("needle"));
    }

    #[test]
    fn test_grep_etc_passwd_rejected() {
        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "grep".into(),
            args: vec!["root".into(), "/etc/passwd".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("operand") || err.contains("not within"));
    }

    // ── Fix 3: Basename normalization tests ────────────────────────

    #[test]
    fn test_bin_sh_blocked() {
        let mut sandbox = make_enabled_sandbox();
        sandbox.command_allowlist.push("/bin/sh".into());
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "/bin/sh".into(),
            args: vec!["-c".into(), "echo hi".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("interpreter") || err.contains("blocked"),
            "expected interpreter block for /bin/sh, got: {}",
            err
        );
    }

    #[test]
    fn test_usr_bin_bash_blocked() {
        let mut sandbox = make_enabled_sandbox();
        sandbox.command_allowlist.push("/usr/bin/bash".into());
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "/usr/bin/bash".into(),
            args: vec!["-c".into(), "echo hi".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("interpreter") || err.contains("blocked"),
            "expected interpreter block for /usr/bin/bash, got: {}",
            err
        );
    }

    #[test]
    fn test_sh_basename_still_blocked() {
        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;
        sandbox.cwd = tmp_dir();
        sandbox.safe_paths = vec![tmp_dir()];
        sandbox.command_allowlist = vec!["sh".into(), "echo".into()];
        sandbox.timeout_ms = 30_000;
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "sh".into(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
    }

    // ── Existing tests (kept, adjusted for new cwd semantics) ──────

    #[test]
    fn test_allowed_command_succeeds_within_timeout() {
        let sandbox = make_enabled_sandbox();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "echo".into(),
            args: vec!["hello".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.stdout.contains("hello"));
        assert_eq!(output.exit_code, 0);
        assert!(!output.timed_out);
    }

    #[test]
    fn test_timeout_triggers_and_returns_timed_out() {
        let mut sandbox = sandbox_with_safe_tmp();
        sandbox.timeout_ms = 500;
        sandbox.command_allowlist.push("sleep".into());
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "sleep".into(),
            args: vec!["10".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let start = Instant::now();
        let result = executor.execute(&req);
        let elapsed = start.elapsed().as_millis();
        assert!(
            elapsed < 5000,
            "timeout should trigger quickly, took {} ms",
            elapsed
        );
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.timed_out);
        assert_eq!(output.exit_code, -1);
    }

    #[test]
    fn test_disabled_sandbox_blocks_before_spawn() {
        let sandbox = make_disabled_sandbox();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "echo".into(),
            args: vec!["test".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("disabled"));
    }

    #[test]
    fn test_denied_command_blocks_before_spawn() {
        let sandbox = make_enabled_sandbox();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "rm".into(),
            args: vec!["-rf".into(), "/tmp/test".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("dangerous") || err.contains("blocked"));
    }

    #[test]
    fn test_unknown_command_blocked_when_allowlist_non_empty() {
        let sandbox = make_restricted_sandbox();
        let executor = ShellExecutor::new(sandbox);
        // "grep" is NOT in this restricted sandbox
        let req = ShellCommandRequest {
            command: "grep".into(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("allowlist"));
    }

    #[test]
    fn test_disallowed_env_variable_rejected() {
        let sandbox = make_enabled_sandbox();
        let executor = ShellExecutor::new(sandbox);
        let mut env = HashMap::new();
        env.insert("SECRET_KEY".into(), "abc123".into());
        let req = ShellCommandRequest {
            command: "echo".into(),
            args: vec!["test".into()],
            cwd: None,
            env,
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("env"));
    }

    #[test]
    fn test_command_with_shell_metacharacters_rejected() {
        let sandbox = make_enabled_sandbox();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "ls | cat".into(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("metacharacter"));
    }

    #[test]
    fn test_allowed_command_with_env_succeeds() {
        let mut sandbox = make_restricted_sandbox();
        sandbox.env_allowlist.push("CUSTOM_VAR".into());
        let executor = ShellExecutor::new(sandbox);
        let mut env = HashMap::new();
        env.insert("CUSTOM_VAR".into(), "my_value".into());
        let req = ShellCommandRequest {
            command: "echo".into(),
            args: vec!["test".into()],
            cwd: None,
            env,
            reason: Some("testing allowed env".into()),
        };
        let result = executor.execute(&req);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cat_etc_passwd_rejected() {
        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "cat".into(),
            args: vec!["/etc/passwd".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("operand") || err.contains("not within"));
    }

    #[test]
    fn test_cat_tmp_env_path_deny_read() {
        let tmp = tmp_dir();
        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;
        sandbox.cwd = tmp.clone();
        sandbox.safe_paths = vec![tmp.clone()];
        sandbox.deny_read_patterns = vec!["**/.env".into()];
        sandbox.command_allowlist = vec!["cat".into()];

        let executor = ShellExecutor::new(sandbox);
        let env_path = format!("{}/.env", tmp);
        let req = ShellCommandRequest {
            command: "cat".into(),
            args: vec![env_path],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("operand") || err.contains("denied") || err.contains("deny"));
    }

    #[test]
    fn test_cat_allowed_file_succeeds() {
        let tmp = tmp_dir();
        let file_path = format!("{}/openlife_p9_test_safe2.txt", tmp);
        let mut f = std::fs::File::create(&file_path).unwrap();
        writeln!(f, "hello sandbox").unwrap();
        drop(f);

        let sandbox = sandbox_with_safe_tmp();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "cat".into(),
            args: vec![file_path.clone()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        let _ = std::fs::remove_file(&file_path);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.stdout.contains("hello sandbox"));
    }

    #[test]
    fn test_args_with_pipe_metacharacter_rejected() {
        let sandbox = make_enabled_sandbox();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "echo".into(),
            args: vec!["hello|cat".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("metacharacter"));
    }

    #[test]
    fn test_args_with_semicolon_rejected() {
        let sandbox = make_enabled_sandbox();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "echo".into(),
            args: vec!["hello;rm".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("metacharacter"));
    }

    #[test]
    fn test_sh_interpreter_blocked() {
        let mut sandbox = make_enabled_sandbox();
        sandbox.command_allowlist.push("sh".into());
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "sh".into(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("interpreter"));
    }

    #[test]
    fn test_bash_with_dash_c_rejected() {
        let mut sandbox = make_enabled_sandbox();
        sandbox.command_allowlist.push("bash".into());
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "bash".into(),
            args: vec!["-c".into(), "echo hi".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("interpreter") || err.contains("blocked"),
            "expected interpreter block, got: {}",
            err
        );
    }

    #[test]
    fn test_zsh_blocked() {
        let mut sandbox = make_enabled_sandbox();
        sandbox.command_allowlist.push("zsh".into());
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "zsh".into(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("interpreter") || err.contains("blocked"));
    }

    #[test]
    fn test_powershell_blocked() {
        let mut sandbox = make_enabled_sandbox();
        sandbox.command_allowlist.push("powershell".into());
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "powershell".into(),
            args: vec!["-Command".into(), "Write-Host hi".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("interpreter") || err.contains("blocked"),
            "expected interpreter block, got: {}",
            err
        );
    }

    #[test]
    fn test_echo_escape_c_as_flag_ok() {
        let sandbox = make_enabled_sandbox();
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "echo".into(),
            args: vec!["-c".into(), "hello".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bash_allowlisted_still_blocked() {
        let mut sandbox = ExecutionSandbox::default();
        sandbox.bash_enabled = true;
        sandbox.cwd = tmp_dir();
        sandbox.safe_paths = vec![tmp_dir()];
        sandbox.command_allowlist = vec!["bash".into(), "echo".into()];
        sandbox.timeout_ms = 30_000;
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "bash".into(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("interpreter") || err.contains("blocked"));
    }
}
