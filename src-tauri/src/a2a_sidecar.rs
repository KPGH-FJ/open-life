use crate::errors::AppError;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

pub struct A2ASidecar {
    child: Mutex<Option<Child>>,
    port: u16,
}

impl A2ASidecar {
    pub fn new(port: u16) -> Self {
        Self {
            child: Mutex::new(None),
            port,
        }
    }

    pub async fn start(&self) -> Result<(), AppError> {
        {
            let child_lock = self
                .child
                .lock()
                .map_err(|e| AppError::internal(format!("mutex poison: {}", e)))?;
            if child_lock.is_some() {
                println!("[A2A Sidecar] already running - a2a_sidecar.rs:20");
                return Ok(());
            }
        }

        match crate::a2a_server::classify_local_sidecar(self.port).await {
            crate::a2a_server::LocalSidecarStatus::Current => {
                println!(
                    "[A2A Sidecar] reusing current OpenLife A2A sidecar on port {} - a2a_sidecar.rs:28",
                    self.port
                );
                return Ok(());
            }
            crate::a2a_server::LocalSidecarStatus::NotRunning => {}
            status => {
                let detail = status
                    .mismatch_detail()
                    .unwrap_or_else(|| status.status_label());
                return Err(AppError::internal(format!(
                    "Refusing to reuse A2A sidecar on port {}: {}",
                    self.port, detail
                )));
            }
        }

        let bin_path = resolve_a2a_server_binary()?;
        println!(
            "[A2A Sidecar] starting binary: {:?} - a2a_sidecar.rs:33",
            bin_path
        );

        let child = Command::new(&bin_path)
            .env("A2A_PORT", self.port.to_string())
            .env("OPENLIFE_PROFILE", crate::storage::openlife_profile())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AppError::internal(format!("Failed to spawn A2A sidecar: {}", e)))?;

        let mut child_lock = self
            .child
            .lock()
            .map_err(|e| AppError::internal(format!("mutex poison: {}", e)))?;
        *child_lock = Some(child);
        println!(
            "[A2A Sidecar] spawned on port {} - a2a_sidecar.rs:43",
            self.port
        );
        Ok(())
    }

    pub fn stop(&self) -> Result<(), AppError> {
        let mut child_lock = self
            .child
            .lock()
            .map_err(|e| AppError::internal(format!("mutex poison: {}", e)))?;
        if let Some(mut child) = child_lock.take() {
            let _ = child.kill();
            println!("[A2A Sidecar] stopped - a2a_sidecar.rs:43");
        }
        Ok(())
    }
}

impl Drop for A2ASidecar {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn resolve_a2a_server_binary() -> Result<std::path::PathBuf, AppError> {
    // 1. env override
    if let Ok(path) = std::env::var("A2A_SERVER_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    // 2. development path (workspace target/debug)
    let dev_path = std::env::current_dir()
        .map_err(|e| AppError::internal(format!("current_dir failed: {}", e)))?
        .join("target")
        .join("debug")
        .join("openlife-a2a-server");
    if dev_path.exists() {
        return Ok(dev_path);
    }

    // 3. same directory as current executable (production sidecar)
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
