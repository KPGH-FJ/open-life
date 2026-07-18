use crate::errors::AppError;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone)]
pub struct A2ASidecar {
    inner: Arc<A2ASidecarInner>,
}

struct A2ASidecarInner {
    child: Mutex<Option<Child>>,
    port: u16,
    start_in_progress: AtomicBool,
    desired_running: AtomicBool,
    reused_external_process: AtomicBool,
}

struct StartPermit<'a>(&'a AtomicBool);

impl Drop for StartPermit<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl A2ASidecar {
    pub fn new(port: u16) -> Self {
        Self {
            inner: Arc::new(A2ASidecarInner {
                child: Mutex::new(None),
                port,
                start_in_progress: AtomicBool::new(false),
                desired_running: AtomicBool::new(false),
                reused_external_process: AtomicBool::new(false),
            }),
        }
    }

    fn acquire_start_permit(&self) -> Result<StartPermit<'_>, AppError> {
        self.inner
            .start_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| AppError::internal("A2A sidecar start is already in progress"))?;
        Ok(StartPermit(&self.inner.start_in_progress))
    }

    fn existing_child_is_running(&self) -> Result<bool, AppError> {
        let mut child = self
            .inner
            .child
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(process) = child.as_mut() else {
            return Ok(false);
        };
        match process.try_wait() {
            Ok(None) => Ok(true),
            Ok(Some(_)) => {
                child.take();
                Ok(false)
            }
            Err(error) => Err(AppError::internal(format!(
                "failed to inspect A2A sidecar process: {error}"
            ))),
        }
    }

    fn install_spawned_child(&self, child: Child) -> Result<(), AppError> {
        let mut child_slot = self
            .inner
            .child
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let rejection = if !self.inner.desired_running.load(Ordering::Acquire) {
            Some("A2A sidecar start was cancelled during process spawn")
        } else if child_slot.is_some() {
            Some("A2A sidecar child appeared during guarded start")
        } else {
            None
        };
        if let Some(reason) = rejection {
            // Linearize the desired-running check with `stop()` through the
            // child mutex, then release the mutex before the blocking reap.
            drop(child_slot);
            terminate_owned_child(child)?;
            return Err(AppError::internal(reason));
        }
        *child_slot = Some(child);
        self.inner
            .reused_external_process
            .store(false, Ordering::Release);
        Ok(())
    }

    fn record_reused_external_process(&self) -> Result<(), AppError> {
        let child_slot = self
            .inner
            .child
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !self.inner.desired_running.load(Ordering::Acquire) {
            return Err(AppError::internal(
                "A2A sidecar start was cancelled while validating an existing process",
            ));
        }
        if child_slot.is_some() {
            return Err(AppError::internal(
                "A2A sidecar child appeared while validating an existing process",
            ));
        }
        self.inner
            .reused_external_process
            .store(true, Ordering::Release);
        Ok(())
    }

    pub async fn start(&self) -> Result<(), AppError> {
        let pairing_token = crate::a2a_server::require_authenticated_dev_a2a_opt_in()
            .map_err(AppError::permission)?;
        let _permit = self.acquire_start_permit()?;
        self.inner.desired_running.store(true, Ordering::Release);
        if self.existing_child_is_running()? {
            self.inner
                .reused_external_process
                .store(false, Ordering::Release);
            return Ok(());
        }

        match crate::a2a_server::classify_local_sidecar(self.inner.port, &pairing_token).await {
            crate::a2a_server::LocalSidecarStatus::Current => {
                return self.record_reused_external_process();
            }
            crate::a2a_server::LocalSidecarStatus::NotRunning => {}
            status => {
                self.inner.desired_running.store(false, Ordering::Release);
                let detail = status
                    .mismatch_detail()
                    .unwrap_or_else(|| status.status_label());
                return Err(AppError::internal(format!(
                    "Refusing to reuse A2A sidecar on port {}: {}",
                    self.inner.port, detail
                )));
            }
        }

        if !self.inner.desired_running.load(Ordering::Acquire) {
            return Err(AppError::internal(
                "A2A sidecar start was cancelled before process spawn",
            ));
        }
        let bin_path = resolve_a2a_server_binary()?;
        let child = Command::new(&bin_path)
            .env("A2A_PORT", self.inner.port.to_string())
            .env("OPENLIFE_PROFILE", crate::storage::openlife_profile())
            .env("OPENLIFE_ENABLE_DEV_A2A", "1")
            .env("OPENLIFE_A2A_PAIRED_TOKEN", pairing_token)
            .env(crate::a2a_server::A2A_PARENT_PIPE_GUARD_ENV, "1")
            // The child owns the read end while `Child` retains this pipe's
            // write end. The OS closes it even if the desktop parent crashes
            // or is killed, so the sidecar cannot survive as an orphan.
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                self.inner.desired_running.store(false, Ordering::Release);
                AppError::internal(format!("Failed to spawn A2A sidecar: {error}"))
            })?;

        self.install_spawned_child(child)
    }

    pub fn stop(&self) -> Result<(), AppError> {
        self.inner.desired_running.store(false, Ordering::Release);
        let mut child = self
            .inner
            .child
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(process) = child.take() {
            terminate_owned_child(process)?;
            self.inner
                .reused_external_process
                .store(false, Ordering::Release);
            return Ok(());
        }
        if self.inner.reused_external_process.load(Ordering::Acquire) {
            return Err(AppError::permission(
                "A2A sidecar is authenticated but owned by another process; OpenLife did not stop it",
            ));
        }
        Ok(())
    }
}

impl Drop for A2ASidecarInner {
    fn drop(&mut self) {
        let child = self
            .child
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(process) = child.take() {
            let _ = terminate_owned_child(process);
        }
    }
}

fn terminate_owned_child(mut process: Child) -> Result<std::process::ExitStatus, AppError> {
    // `kill` may race with a natural exit. Always call `wait` regardless:
    // dropping `Child` does not reap a Unix child process.
    let _ = process.kill();
    process
        .wait()
        .map_err(|error| AppError::internal(format!("failed to reap A2A child: {error}")))
}

fn resolve_a2a_server_binary() -> Result<std::path::PathBuf, AppError> {
    if let Ok(path) = std::env::var("A2A_SERVER_PATH") {
        let path = std::path::PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }

    let dev_path = std::env::current_dir()
        .map_err(|error| AppError::internal(format!("current_dir failed: {error}")))?
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_start_permits_are_single_owner_without_an_async_guard() {
        let sidecar = A2ASidecar::new(8766);
        let first = sidecar.acquire_start_permit().unwrap();
        assert!(sidecar.acquire_start_permit().is_err());
        drop(first);
        assert!(sidecar.acquire_start_permit().is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn cancelled_spawn_path_kills_and_reaps_the_owned_child() {
        let sidecar = A2ASidecar::new(8766);
        let child = Command::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn deterministic child fixture");
        let started = std::time::Instant::now();
        let error = sidecar
            .install_spawned_child(child)
            .expect_err("default desired-running=false must take the cancellation branch");
        assert!(error.to_string().contains("cancelled during process spawn"));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "kill+wait must reap instead of waiting for the fixture's natural exit"
        );
    }

    #[cfg(unix)]
    #[test]
    fn poisoned_child_mutex_cannot_bypass_spawn_cancellation_reaping() {
        let sidecar = A2ASidecar::new(8766);
        let poison_sidecar = sidecar.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poison_sidecar.inner.child.lock().unwrap();
            panic!("poison child mutex fixture");
        })
        .join();
        assert!(sidecar.inner.child.is_poisoned());

        let child = Command::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn poison-path child fixture");
        let started = std::time::Instant::now();
        let error = sidecar
            .install_spawned_child(child)
            .expect_err("poison recovery must still observe desired-running=false");
        assert!(error.to_string().contains("cancelled during process spawn"));
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(
            sidecar
                .inner
                .child
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none(),
            "poison recovery must not retain the rejected child"
        );
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_stop_and_spawn_adoption_leave_no_owned_child() {
        for _ in 0..24 {
            let sidecar = A2ASidecar::new(8766);
            sidecar.inner.desired_running.store(true, Ordering::Release);
            let child = Command::new("/bin/sleep")
                .arg("30")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn race child fixture");
            let barrier = Arc::new(std::sync::Barrier::new(3));

            let install_sidecar = sidecar.clone();
            let install_barrier = Arc::clone(&barrier);
            let install = std::thread::spawn(move || {
                install_barrier.wait();
                install_sidecar.install_spawned_child(child)
            });
            let stop_sidecar = sidecar.clone();
            let stop_barrier = Arc::clone(&barrier);
            let stop = std::thread::spawn(move || {
                stop_barrier.wait();
                stop_sidecar.stop()
            });

            barrier.wait();
            let install_result = install.join().expect("join spawn adoption");
            stop.join().expect("join concurrent stop").unwrap();
            if let Err(error) = install_result {
                assert!(
                    error.to_string().contains("cancelled during process spawn"),
                    "unexpected install terminal: {error}"
                );
            }
            assert!(!sidecar.inner.desired_running.load(Ordering::Acquire));
            assert!(
                sidecar
                    .inner
                    .child
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_none(),
                "the start/stop linearization must leave no owned child"
            );
        }
    }

    #[test]
    fn concurrent_stop_and_external_reuse_have_one_linearized_outcome() {
        for _ in 0..32 {
            let sidecar = A2ASidecar::new(8766);
            sidecar.inner.desired_running.store(true, Ordering::Release);
            let barrier = Arc::new(std::sync::Barrier::new(3));
            let reuse_sidecar = sidecar.clone();
            let reuse_barrier = Arc::clone(&barrier);
            let reuse = std::thread::spawn(move || {
                reuse_barrier.wait();
                reuse_sidecar.record_reused_external_process()
            });
            let stop_sidecar = sidecar.clone();
            let stop_barrier = Arc::clone(&barrier);
            let stop = std::thread::spawn(move || {
                stop_barrier.wait();
                stop_sidecar.stop()
            });

            barrier.wait();
            let reuse_result = reuse.join().expect("join external reuse");
            let stop_result = stop.join().expect("join external stop");
            match (reuse_result, stop_result) {
                (Ok(()), Err(error)) => {
                    assert!(error.to_string().contains("owned by another process"));
                    assert!(sidecar.inner.reused_external_process.load(Ordering::Acquire));
                }
                (Err(error), Ok(())) => {
                    assert!(error
                        .to_string()
                        .contains("cancelled while validating an existing process"));
                    assert!(!sidecar.inner.reused_external_process.load(Ordering::Acquire));
                }
                (reuse, stop) => panic!(
                    "start/stop must have one external ownership result: reuse={reuse:?}, stop={stop:?}"
                ),
            }
            assert!(!sidecar.inner.desired_running.load(Ordering::Acquire));
        }
    }
}
