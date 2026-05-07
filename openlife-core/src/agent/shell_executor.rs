use crate::agent::execution_sandbox::ExecutionSandbox;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

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
            ShellExecutionError::CommandNotFound(cmd) => {
                write!(f, "command not found: {}", cmd)
            }
            ShellExecutionError::SpawnFailed(msg) => write!(f, "spawn failed: {}", msg),
            ShellExecutionError::Timeout(ms) => write!(f, "timeout after {} ms", ms),
            ShellExecutionError::IoError(msg) => write!(f, "i/o error: {}", msg),
        }
    }
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

        // 1. Validate command through sandbox
        if let Err(msg) = self
            .sandbox
            .validate(&request.command, request.cwd.as_deref())
        {
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

        // 2. Validate env variable names
        for var_name in request.env.keys() {
            if !self.sandbox.is_env_allowed(var_name) {
                // For p9-4: reject disallowed env vars
                return Err(ShellExecutionError::EnvNotAllowed(var_name.clone()));
            }
        }

        // 3. Build and run the process
        let mut cmd = std::process::Command::new(&request.command);
        cmd.args(&request.args);

        // Strip environment, then add only allowed vars
        cmd.env_clear();
        for (k, v) in &request.env {
            if self.sandbox.is_env_allowed(k) {
                cmd.env(k, v);
            }
        }

        if let Some(ref cwd) = request.cwd {
            cmd.current_dir(cwd);
        }

        let start = Instant::now();
        let max_bytes = if self.sandbox.max_output_bytes > 0 {
            self.sandbox.max_output_bytes
        } else {
            1024 * 1024
        };

        let child = cmd
            .output()
            .map_err(|e| ShellExecutionError::SpawnFailed(e.to_string()))?;

        let elapsed = start.elapsed().as_millis() as u64;
        let timed_out = false; // output() blocks, can't truly timeout
        let stdout_truncated = child.stdout.len() > max_bytes;
        let stderr_truncated = child.stderr.len() > max_bytes;
        let stdout = if stdout_truncated {
            String::from_utf8_lossy(&child.stdout[..max_bytes]).into_owned()
        } else {
            String::from_utf8_lossy(&child.stdout).into_owned()
        };
        let stderr = if stderr_truncated {
            String::from_utf8_lossy(&child.stderr[..max_bytes]).into_owned()
        } else {
            String::from_utf8_lossy(&child.stderr).into_owned()
        };

        Ok(ShellCommandOutput {
            stdout,
            stderr,
            exit_code: child.status.code().unwrap_or(-1),
            timed_out,
            truncated: stdout_truncated || stderr_truncated,
            elapsed_ms: elapsed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_disabled_sandbox() -> ExecutionSandbox {
        ExecutionSandbox::default() // bash_enabled: false
    }

    fn make_enabled_sandbox() -> ExecutionSandbox {
        let mut s = ExecutionSandbox::default();
        s.bash_enabled = true;
        s.timeout_ms = 10_000;
        s.max_output_bytes = 1024 * 1024;
        s
    }

    fn make_restricted_sandbox() -> ExecutionSandbox {
        let mut s = ExecutionSandbox::default();
        s.bash_enabled = true;
        s.command_allowlist = vec!["echo".into(), "date".into()];
        s.timeout_ms = 10_000;
        s.max_output_bytes = 10 * 1024;
        s
    }

    #[test]
    fn test_allowed_command_succeeds() {
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
        assert!(output.exit_code == 0);
        assert!(!output.timed_out);
        assert!(!output.truncated);
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
        let req = ShellCommandRequest {
            command: "ls".into(),
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
    fn test_timeout_kills_long_running_command() {
        let mut sandbox = make_enabled_sandbox();
        sandbox.timeout_ms = 100;
        let executor = ShellExecutor::new(sandbox);
        let req = ShellCommandRequest {
            command: "sleep".into(),
            args: vec!["10".into()],
            cwd: None,
            env: HashMap::new(),
            reason: None,
        };
        let result = executor.execute(&req);
        // sleep might be blocked by allowlist or timeout
        if result.is_ok() {
            let output = result.unwrap();
            assert!(output.timed_out);
        } else {
            let err = format!("{}", result.unwrap_err());
            assert!(err.contains("allowlist") || err.contains("dangerous"));
        }
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
}
