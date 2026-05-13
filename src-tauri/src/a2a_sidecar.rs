use crate::errors::AppError;
use std::process::{Child, Command, Stdio};

pub struct A2ASidecar {
    child: Option<Child>,
    port: u16,
}

impl A2ASidecar {
    pub fn new(port: u16) -> Self {
        Self { child: None, port }
    }

    pub async fn start(&mut self) -> Result<(), AppError> {
        if self.child.is_some() {
            println!("[A2A Sidecar] already running - a2a_sidecar.rs:20");
            return Ok(());
        }

        if crate::a2a_server::has_reachable_local_server(self.port).await {
            println!(
                "[A2A Sidecar] detected existing OpenLife A2A server on port {} - a2a_sidecar.rs:26",
                self.port
            );
            return Ok(());
        }

        let bin_path = resolve_a2a_server_binary()?;
        println!(
            "[A2A Sidecar] starting binary: {:?} - a2a_sidecar.rs:33",
            bin_path
        );

        let token = resolve_a2a_token();
        let instance_id = resolve_a2a_instance_id();
        let child = Command::new(&bin_path)
            .env("A2A_PORT", self.port.to_string())
            .env("A2A_BEARER_TOKEN", &token)
            .env("A2A_INSTANCE_ID", &instance_id)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AppError::internal(format!("Failed to spawn A2A sidecar: {}", e)))?;

        self.child = Some(child);
        println!(
            "[A2A Sidecar] spawned on port {} - a2a_sidecar.rs:43",
            self.port
        );
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            println!("[A2A Sidecar] stopped - a2a_sidecar.rs:43");
        }
    }
}

impl Drop for A2ASidecar {
    fn drop(&mut self) {
        self.stop();
    }
}

fn resolve_a2a_token() -> String {
    let path = crate::storage::app_data_dir().join("a2a_token");
    if path.exists() {
        if let Ok(token) = std::fs::read_to_string(&path) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return token;
            }
        }
    }
    // Generate if missing
    let token = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::write(&path, &token);
    token
}

fn resolve_a2a_instance_id() -> String {
    let path = crate::storage::app_data_dir().join("a2a_instance_id");
    if path.exists() {
        if let Ok(id) = std::fs::read_to_string(&path) {
            let id = id.trim().to_string();
            if !id.is_empty() {
                return id;
            }
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::write(&path, &id);
    id
}

fn resolve_a2a_server_binary() -> Result<std::path::PathBuf, AppError> {
    if let Ok(path) = std::env::var("A2A_SERVER_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    let dev_path = std::env::current_dir()
        .map_err(|e| AppError::internal(format!("current_dir failed: {}", e)))?
        .join("target")
        .join("debug")
        .join("openlife-a2a-server");
    if dev_path.exists() {
        return Ok(dev_path);
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sidecar = if cfg!(windows) {
                dir.join("openlife-a2a-server.exe")
            } else {
                dir.join("openlife-a2a-server")
            };
            if sidecar.exists() {
                return Ok(sidecar);
            }
        }
    }

    Err(AppError::internal(
        "A2A server binary not found. Set A2A_SERVER_PATH or ensure openlife-a2a-server is built.",
    ))
}
