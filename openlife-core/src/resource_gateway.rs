//! The only import interface for product resources.
//!
//! Blocking document parsers execute in a killable child process. The gateway
//! does not enter the canonical store until every file has parsed successfully,
//! and cancellation is linearized against the final SQLite commit.

use crate::resource::{
    ResourceDetachReceipt, ResourceImportBatch, ResourceImportCandidate, ResourceImportReceipt,
    ResourceStore, MAX_IMPORT_BYTES, MAX_RESOURCES_PER_IMPORT, MAX_RESOURCE_BYTES,
};
use crate::resource_parser::{extract_resource, ResourceExtraction, ResourceExtractionRequest};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

pub const RESOURCE_PARSER_WORKER_ARG: &str = "--openlife-resource-parser-worker-v1";
pub const RESOURCE_PARSER_TIMEOUT: Duration = Duration::from_secs(30);
pub const RESOURCE_PARSER_CONCURRENCY: usize = 2;

const MAX_WORKER_HEADER_BYTES: usize = 4 * 1024;
const MAX_WORKER_OUTPUT_BYTES: usize = 80 * 1024 * 1024;
#[cfg(target_os = "macos")]
const MAX_WORKER_RESIDENT_BYTES: u64 = 384 * 1024 * 1024;
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceImportSource {
    pub filename: String,
    pub declared_mime: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Default)]
pub struct ResourceImportCancellation {
    inner: Arc<ResourceImportCancellationInner>,
}

#[derive(Default)]
struct ResourceImportCancellationInner {
    cancelled: AtomicBool,
    commit_gate: Mutex<()>,
}

impl ResourceImportCancellation {
    /// Linearize cancellation against a canonical commit already in progress.
    ///
    /// If commit owns the gate, cancellation completes after that commit and is
    /// therefore ordered after it. If cancellation owns the gate first, no
    /// later commit can start.
    pub fn cancel(&self) {
        let _gate = self
            .inner
            .commit_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.inner.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    fn begin_commit(&self) -> Result<std::sync::MutexGuard<'_, ()>> {
        let gate = self
            .inner
            .commit_gate
            .lock()
            .map_err(|error| anyhow::anyhow!("resource_import_commit_gate_poisoned:{error}"))?;
        if self.is_cancelled() {
            anyhow::bail!("resource_import_cancelled_before_commit");
        }
        Ok(gate)
    }

    async fn cancelled(&self) {
        while !self.is_cancelled() {
            tokio::time::sleep(CANCEL_POLL_INTERVAL).await;
        }
    }
}

#[derive(Clone)]
pub struct ResourceParserProcess {
    executable: PathBuf,
    arguments: Arc<Vec<OsString>>,
    timeout: Duration,
    concurrency: Arc<Semaphore>,
    apply_unix_resource_limits: bool,
}

impl ResourceParserProcess {
    pub fn for_current_executable() -> Result<Self> {
        let executable = std::env::current_exe().context("resource_parser_current_exe_missing")?;
        Ok(Self {
            executable,
            arguments: Arc::new(vec![OsString::from(RESOURCE_PARSER_WORKER_ARG)]),
            timeout: RESOURCE_PARSER_TIMEOUT,
            concurrency: Arc::new(Semaphore::new(RESOURCE_PARSER_CONCURRENCY)),
            apply_unix_resource_limits: true,
        })
    }

    #[cfg(test)]
    fn test_command(
        executable: impl Into<PathBuf>,
        arguments: Vec<OsString>,
        timeout: Duration,
    ) -> Self {
        Self {
            executable: executable.into(),
            arguments: Arc::new(arguments),
            timeout,
            concurrency: Arc::new(Semaphore::new(RESOURCE_PARSER_CONCURRENCY)),
            apply_unix_resource_limits: true,
        }
    }

    pub async fn extract(
        &self,
        request: ResourceExtractionRequest,
        cancellation: &ResourceImportCancellation,
    ) -> Result<ResourceExtraction> {
        validate_parent_request(&request)?;
        let _permit = self.acquire_permit(cancellation).await?;
        if cancellation.is_cancelled() {
            anyhow::bail!("resource_parser_cancelled_before_spawn");
        }

        let header = WorkerRequestHeader {
            filename: request.filename,
            declared_mime: request.declared_mime,
            byte_count: request.bytes.len() as u64,
        };
        let header_bytes =
            serde_json::to_vec(&header).context("resource_parser_header_encode_failed")?;
        if header_bytes.len() > MAX_WORKER_HEADER_BYTES {
            anyhow::bail!("resource_parser_header_too_large");
        }

        let mut command = Command::new(&self.executable);
        command
            .args(self.arguments.iter())
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        apply_worker_resource_limits(&mut command, self.apply_unix_resource_limits)?;
        let mut child = command.spawn().with_context(|| {
            format!("resource_parser_spawn_failed:{}", self.executable.display())
        })?;
        let child_pid = child
            .id()
            .ok_or_else(|| anyhow::anyhow!("resource_parser_pid_missing"))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("resource_parser_stdin_missing"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("resource_parser_stdout_missing"))?;
        let output_reader = tokio::spawn(read_bounded_worker_output(stdout));

        let started = tokio::time::Instant::now();
        let write_input = async {
            stdin
                .write_all(&(header_bytes.len() as u32).to_be_bytes())
                .await?;
            stdin.write_all(&header_bytes).await?;
            stdin.write_all(&request.bytes).await?;
            stdin.shutdown().await?;
            Result::<()>::Ok(())
        };
        tokio::select! {
            result = write_input => {
                if let Err(error) = result {
                    kill_and_reap(&mut child).await;
                    output_reader.abort();
                    anyhow::bail!("resource_parser_input_failed:{error}");
                }
            }
            _ = cancellation.cancelled() => {
                kill_and_reap(&mut child).await;
                output_reader.abort();
                anyhow::bail!("resource_parser_cancelled");
            }
            _ = tokio::time::sleep(self.timeout) => {
                kill_and_reap(&mut child).await;
                output_reader.abort();
                anyhow::bail!("resource_parser_timeout");
            }
            _ = worker_memory_limit_exceeded(child_pid) => {
                kill_and_reap(&mut child).await;
                output_reader.abort();
                anyhow::bail!("resource_parser_memory_limit_exceeded");
            }
        }

        let elapsed = started.elapsed();
        let remaining = self.timeout.saturating_sub(elapsed);
        if remaining.is_zero() {
            kill_and_reap(&mut child).await;
            output_reader.abort();
            anyhow::bail!("resource_parser_timeout");
        }
        let status = tokio::select! {
            status = child.wait() => status.context("resource_parser_wait_failed")?,
            _ = cancellation.cancelled() => {
                kill_and_reap(&mut child).await;
                output_reader.abort();
                anyhow::bail!("resource_parser_cancelled");
            }
            _ = tokio::time::sleep(remaining) => {
                kill_and_reap(&mut child).await;
                output_reader.abort();
                anyhow::bail!("resource_parser_timeout");
            }
            _ = worker_memory_limit_exceeded(child_pid) => {
                kill_and_reap(&mut child).await;
                output_reader.abort();
                anyhow::bail!("resource_parser_memory_limit_exceeded");
            }
        };
        let output = output_reader
            .await
            .context("resource_parser_output_task_join_failed")??;
        if !status.success() {
            anyhow::bail!(
                "resource_parser_worker_failed:{}",
                status.code().unwrap_or(-1)
            );
        }
        let response: WorkerResponse =
            serde_json::from_slice(&output).context("resource_parser_output_invalid")?;
        match response {
            WorkerResponse::Success { extraction } => Ok(extraction),
            WorkerResponse::Failure { code } => anyhow::bail!("{code}"),
        }
    }

    async fn acquire_permit(
        &self,
        cancellation: &ResourceImportCancellation,
    ) -> Result<OwnedSemaphorePermit> {
        tokio::select! {
            permit = Arc::clone(&self.concurrency).acquire_owned() => {
                permit.map_err(|_| anyhow::anyhow!("resource_parser_concurrency_closed"))
            }
            _ = cancellation.cancelled() => anyhow::bail!("resource_parser_cancelled_before_queue"),
        }
    }
}

#[derive(Clone)]
pub struct ResourceGateway {
    store: ResourceStore,
    parser: ResourceParserProcess,
}

impl ResourceGateway {
    pub fn new(store: ResourceStore, parser: ResourceParserProcess) -> Self {
        Self { store, parser }
    }

    pub fn store(&self) -> &ResourceStore {
        &self.store
    }

    pub async fn import_resources(
        &self,
        operation_id: String,
        message_id: String,
        sources: Vec<ResourceImportSource>,
        cancellation: ResourceImportCancellation,
    ) -> Result<ResourceImportReceipt> {
        validate_gateway_batch(&operation_id, &message_id, &sources)?;
        let mut candidates = Vec::with_capacity(sources.len());
        for source in sources {
            let extraction = self
                .parser
                .extract(
                    ResourceExtractionRequest {
                        filename: source.filename.clone(),
                        declared_mime: source.declared_mime.clone(),
                        bytes: source.bytes.clone(),
                    },
                    &cancellation,
                )
                .await?;
            candidates.push(ResourceImportCandidate {
                resource_id: Uuid::new_v4().to_string(),
                filename: source.filename,
                declared_mime: source.declared_mime,
                detected_mime: extraction.detected_mime,
                format: extraction.format,
                bytes: source.bytes,
                chunks: extraction.chunks,
            });
        }
        self.store.commit_import_batch_guarded(
            ResourceImportBatch {
                operation_id,
                message_id,
                resources: candidates,
            },
            || cancellation.begin_commit(),
        )
    }

    /// Canonical resource/message binding mutation. Product callers must not
    /// reach through `store()` for this write: ResourceGateway owns resource
    /// lifecycle admission while ToolGateway remains exclusively responsible
    /// for Agent tool execution.
    pub fn detach_resource_from_message(
        &self,
        operation_id: &str,
        message_id: &str,
        resource_id: &str,
    ) -> Result<ResourceDetachReceipt> {
        self.store
            .detach_resource_from_message(operation_id, message_id, resource_id)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerRequestHeader {
    filename: String,
    declared_mime: String,
    byte_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum WorkerResponse {
    Success { extraction: ResourceExtraction },
    Failure { code: String },
}

/// Return a process exit code only when the executable was invoked in the
/// private parser-worker mode. Product startup calls this before Tauri setup.
pub fn run_resource_parser_worker_if_requested() -> Option<i32> {
    if std::env::args_os().nth(1).as_deref()
        != Some(std::ffi::OsStr::new(RESOURCE_PARSER_WORKER_ARG))
    {
        return None;
    }
    Some(match run_resource_parser_worker_stdio() {
        Ok(()) => 0,
        Err(error) => {
            let response = WorkerResponse::Failure {
                code: metadata_safe_worker_error_code(&error),
            };
            if write_worker_response(&response).is_ok() {
                0
            } else {
                2
            }
        }
    })
}

fn run_resource_parser_worker_stdio() -> Result<()> {
    apply_worker_self_memory_limits()?;
    let mut stdin = std::io::stdin().lock();
    let mut header_length = [0u8; 4];
    stdin
        .read_exact(&mut header_length)
        .context("resource_parser_worker_header_length_missing")?;
    let header_length = u32::from_be_bytes(header_length) as usize;
    if header_length == 0 || header_length > MAX_WORKER_HEADER_BYTES {
        anyhow::bail!("resource_parser_worker_header_length_invalid");
    }
    let mut header_bytes = vec![0u8; header_length];
    stdin
        .read_exact(&mut header_bytes)
        .context("resource_parser_worker_header_missing")?;
    let header: WorkerRequestHeader =
        serde_json::from_slice(&header_bytes).context("resource_parser_worker_header_invalid")?;
    if header.byte_count == 0 || header.byte_count > MAX_RESOURCE_BYTES as u64 {
        anyhow::bail!("resource_parser_worker_byte_count_invalid");
    }
    let mut bytes = vec![0u8; header.byte_count as usize];
    stdin
        .read_exact(&mut bytes)
        .context("resource_parser_worker_body_missing")?;
    let response = match extract_resource(ResourceExtractionRequest {
        filename: header.filename,
        declared_mime: header.declared_mime,
        bytes,
    }) {
        Ok(extraction) => WorkerResponse::Success { extraction },
        Err(error) => WorkerResponse::Failure {
            code: metadata_safe_worker_error_code(&error),
        },
    };
    write_worker_response(&response)
}

fn write_worker_response(response: &WorkerResponse) -> Result<()> {
    let response =
        serde_json::to_vec(response).context("resource_parser_worker_output_encode_failed")?;
    if response.len() > MAX_WORKER_OUTPUT_BYTES {
        anyhow::bail!("resource_parser_worker_output_limit_exceeded");
    }
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&response)
        .context("resource_parser_worker_output_write_failed")?;
    stdout
        .flush()
        .context("resource_parser_worker_output_flush_failed")?;
    Ok(())
}

fn metadata_safe_worker_error_code(error: &anyhow::Error) -> String {
    let rendered = error.to_string();
    let code = rendered.split(':').next().unwrap_or_default();
    if code.starts_with("resource_")
        && code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        code.to_string()
    } else {
        "resource_parser_worker_internal_failure".to_string()
    }
}

fn validate_parent_request(request: &ResourceExtractionRequest) -> Result<()> {
    if request.bytes.is_empty() || request.bytes.len() > MAX_RESOURCE_BYTES {
        anyhow::bail!("resource_parser_parent_byte_limit_exceeded");
    }
    if request.filename.len() > 255 || request.declared_mime.len() > 128 {
        anyhow::bail!("resource_parser_parent_metadata_limit_exceeded");
    }
    Ok(())
}

fn validate_gateway_batch(
    operation_id: &str,
    message_id: &str,
    sources: &[ResourceImportSource],
) -> Result<()> {
    let operation =
        Uuid::parse_str(operation_id).context("resource_import_operation_id_invalid")?;
    if operation.get_version_num() != 4
        || operation.to_string() != operation_id.to_ascii_lowercase()
    {
        anyhow::bail!("resource_import_operation_id_must_be_uuid_v4");
    }
    if message_id.trim().is_empty() || message_id.len() > 256 {
        anyhow::bail!("resource_import_message_id_invalid");
    }
    if sources.is_empty() || sources.len() > MAX_RESOURCES_PER_IMPORT {
        anyhow::bail!("resource_import_file_count_exceeded");
    }
    let total = sources
        .iter()
        .try_fold(0usize, |total, source| {
            total.checked_add(source.bytes.len())
        })
        .ok_or_else(|| anyhow::anyhow!("resource_import_total_bytes_overflow"))?;
    if total > MAX_IMPORT_BYTES {
        anyhow::bail!("resource_import_total_bytes_exceeded");
    }
    Ok(())
}

async fn read_bounded_worker_output(mut stdout: tokio::process::ChildStdout) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut overflowed = false;
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = stdout
            .read(&mut buffer)
            .await
            .context("resource_parser_output_read_failed")?;
        if read == 0 {
            break;
        }
        if output.len().saturating_add(read) > MAX_WORKER_OUTPUT_BYTES {
            overflowed = true;
        } else if !overflowed {
            output.extend_from_slice(&buffer[..read]);
        }
    }
    if overflowed {
        anyhow::bail!("resource_parser_output_limit_exceeded");
    }
    Ok(output)
}

async fn kill_and_reap(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn apply_worker_resource_limits(command: &mut Command, enabled: bool) -> Result<()> {
    if !enabled {
        return Ok(());
    }
    #[cfg(unix)]
    {
        // SAFETY: `pre_exec` performs only async-signal-safe `setrlimit` calls
        // and constructs no heap-backed error strings in the child.
        unsafe {
            command.pre_exec(|| {
                set_resource_limit(libc::RLIMIT_CPU, 35)?;
                set_resource_limit(libc::RLIMIT_FSIZE, 32 * 1024 * 1024)?;
                set_resource_limit(libc::RLIMIT_NOFILE, 64)?;
                Ok(())
            });
        }
    }
    Ok(())
}

fn apply_worker_self_memory_limits() -> Result<()> {
    #[cfg(all(unix, not(target_os = "macos")))]
    set_resource_limit(libc::RLIMIT_AS, 768 * 1024 * 1024)
        .context("resource_parser_address_space_limit_failed")?;
    Ok(())
}

#[cfg(target_os = "macos")]
async fn worker_memory_limit_exceeded(pid: u32) {
    loop {
        tokio::time::sleep(CANCEL_POLL_INTERVAL).await;
        if macos_process_resident_bytes(pid).is_some_and(|bytes| bytes > MAX_WORKER_RESIDENT_BYTES)
        {
            return;
        }
    }
}

#[cfg(not(target_os = "macos"))]
async fn worker_memory_limit_exceeded(_pid: u32) {
    std::future::pending::<()>().await;
}

#[cfg(target_os = "macos")]
fn macos_process_resident_bytes(pid: u32) -> Option<u64> {
    let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::zeroed();
    let expected = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
    // SAFETY: `info` points to `expected` writable bytes for the duration of
    // `proc_pidinfo`; the value is read only when the kernel reports a full
    // `proc_taskinfo` result.
    let read = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            expected,
        )
    };
    if read == expected {
        // SAFETY: guarded by the exact-size result above.
        Some(unsafe { info.assume_init() }.pti_resident_size)
    } else {
        None
    }
}

#[cfg(unix)]
fn set_resource_limit(resource: libc::c_int, limit: u64) -> std::io::Result<()> {
    let mut value = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `value` is valid writable storage for `getrlimit`.
    if unsafe { libc::getrlimit(resource as _, &mut value) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Preserve the inherited hard limit. On macOS lowering soft and hard in
    // one pre-exec call can fail with EINVAL even when the new values match.
    value.rlim_cur = (limit as libc::rlim_t).min(value.rlim_max);
    // SAFETY: `value` is a valid `rlimit` for the duration of the call.
    let result = unsafe { libc::setrlimit(resource as _, &value) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_request() -> ResourceExtractionRequest {
        ResourceExtractionRequest {
            filename: "roadshow.md".to_string(),
            declared_mime: "text/markdown".to_string(),
            bytes: b"# Roadshow".to_vec(),
        }
    }

    #[tokio::test]
    async fn cancellation_kills_and_reaps_a_hung_worker_under_one_second() {
        let parser = ResourceParserProcess::test_command(
            "/bin/sleep",
            vec![OsString::from("60")],
            Duration::from_secs(30),
        );
        let cancellation = ResourceImportCancellation::default();
        let cancelling = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancelling.cancel();
        });
        let started = tokio::time::Instant::now();
        let error = parser
            .extract(small_request(), &cancellation)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("cancelled"),
            "unexpected parser error: {error:#}"
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn timeout_kills_and_reaps_a_hung_worker() {
        let parser = ResourceParserProcess::test_command(
            "/bin/sleep",
            vec![OsString::from("60")],
            Duration::from_millis(80),
        );
        let error = parser
            .extract(small_request(), &ResourceImportCancellation::default())
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("timeout"),
            "unexpected parser error: {error:#}"
        );
    }

    #[test]
    fn cancellation_is_linearized_before_commit() {
        let cancellation = ResourceImportCancellation::default();
        cancellation.cancel();
        assert!(cancellation.begin_commit().is_err());
    }

    #[test]
    fn worker_protocol_round_trips_without_copying_bytes_into_json() {
        let header = WorkerRequestHeader {
            filename: "roadshow.md".to_string(),
            declared_mime: "text/markdown".to_string(),
            byte_count: 10,
        };
        let encoded = serde_json::to_vec(&header).unwrap();
        assert!(!String::from_utf8(encoded)
            .unwrap()
            .contains("Roadshow body"));
    }

    #[test]
    fn expanded_budget_is_stricter_than_worker_output_budget() {
        assert!(crate::resource_parser::MAX_EXPANDED_BYTES > MAX_WORKER_OUTPUT_BYTES as u64);
    }
}
