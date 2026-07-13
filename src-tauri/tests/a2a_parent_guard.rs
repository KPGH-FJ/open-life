#![cfg(feature = "dev-extensions")]

use std::io;
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const PAIRING_TOKEN: &str = "a2a-parent-guard-test-token-00000001";

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    condition()
}

fn cleanup(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn spawn_server_without_policy(
    port: u16,
    data_dir: &std::path::Path,
    parent_pipe_guard: bool,
) -> io::Result<Child> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openlife-a2a-server"));
    command
        .env("OPENLIFE_PROFILE", "dev")
        .env("OPENLIFE_ENABLE_DEV_A2A", "1")
        .env("OPENLIFE_A2A_PAIRED_TOKEN", PAIRING_TOKEN)
        .env("OPENLIFE_DATA_DIR", data_dir)
        .env("OPENLIFE_ALLOW_DEV_EXTENSIONS_WITH_CUSTOM_DATA_DIR", "1")
        .env("A2A_PORT", port.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if parent_pipe_guard {
        command.env(
            openlife_tauri_lib::a2a_server::A2A_PARENT_PIPE_GUARD_ENV,
            "1",
        );
    } else {
        command.env_remove(openlife_tauri_lib::a2a_server::A2A_PARENT_PIPE_GUARD_ENV);
    }
    command.spawn()
}

fn persist_privacy_policy(data_dir: &std::path::Path, yaml: &str) -> io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(data_dir.join("privacy_policy.yaml"), yaml)
}

fn spawn_server(
    port: u16,
    data_dir: &std::path::Path,
    parent_pipe_guard: bool,
) -> io::Result<Child> {
    let policy = openlife_core::privacy::PrivacyPolicy::default()
        .to_yaml()
        .map_err(io::Error::other)?;
    persist_privacy_policy(data_dir, &policy)?;
    spawn_server_without_policy(port, data_dir, parent_pipe_guard)
}

fn reserve_loopback_port() -> io::Result<u16> {
    let reservation = TcpListener::bind("127.0.0.1:0")?;
    let port = reservation.local_addr()?.port();
    drop(reservation);
    Ok(port)
}

fn wait_until_reachable(child: &mut Child, port: u16) -> bool {
    wait_until(Duration::from_secs(5), || {
        if child.try_wait().ok().flatten().is_some() {
            return false;
        }
        TcpStream::connect(("127.0.0.1", port)).is_ok()
    })
}

#[test]
fn spawned_a2a_sidecar_exits_when_its_parent_pipe_closes() -> io::Result<()> {
    let port = reserve_loopback_port()?;
    let data_dir = tempfile::tempdir()?;
    let mut child = spawn_server(port, data_dir.path(), true)?;

    if !wait_until_reachable(&mut child, port) {
        cleanup(&mut child);
        panic!("A2A sidecar did not remain alive long enough to prove the parent guard");
    }

    drop(child.stdin.take());
    let exited = wait_until(Duration::from_secs(3), || {
        child.try_wait().ok().flatten().is_some()
    });
    if !exited {
        cleanup(&mut child);
        panic!("A2A sidecar survived after its parent pipe closed");
    }

    let status = child.wait()?;
    assert!(
        status.success(),
        "parent-guard shutdown was not clean: {status}"
    );
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    Ok(())
}

#[test]
fn standalone_dev_a2a_does_not_infer_parent_ownership_from_stdin() -> io::Result<()> {
    let port = reserve_loopback_port()?;
    let data_dir = tempfile::tempdir()?;
    let mut child = spawn_server(port, data_dir.path(), false)?;

    if !wait_until_reachable(&mut child, port) {
        cleanup(&mut child);
        panic!("standalone dev A2A did not become reachable");
    }
    drop(child.stdin.take());
    let exited = wait_until(Duration::from_millis(300), || {
        child.try_wait().ok().flatten().is_some()
    });
    if exited {
        let status = child.wait()?;
        panic!("standalone dev A2A incorrectly treated stdin as a parent guard: {status}");
    }

    assert!(TcpStream::connect(("127.0.0.1", port)).is_ok());
    cleanup(&mut child);
    Ok(())
}

#[test]
fn actual_a2a_binary_fails_closed_without_a_valid_persisted_privacy_policy() -> io::Result<()> {
    for malformed_policy in [None, Some("enabled: [not-valid-yaml")] {
        let port = reserve_loopback_port()?;
        let data_dir = tempfile::tempdir()?;
        if let Some(policy) = malformed_policy {
            persist_privacy_policy(data_dir.path(), policy)?;
        }
        let mut child = spawn_server_without_policy(port, data_dir.path(), true)?;
        let exited = wait_until(Duration::from_secs(3), || {
            child.try_wait().ok().flatten().is_some()
        });
        if !exited {
            cleanup(&mut child);
            panic!(
                "A2A binary must fail closed when its persisted privacy policy is missing or invalid"
            );
        }
        let status = child.wait()?;
        assert!(
            !status.success(),
            "invalid privacy state started A2A: {status}"
        );
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    }
    Ok(())
}

#[tokio::test]
async fn actual_a2a_binary_owns_the_authenticated_bounded_router_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let port = reserve_loopback_port()?;
    let data_dir = tempfile::tempdir()?;
    let mut child = spawn_server(port, data_dir.path(), true)?;
    if !wait_until_reachable(&mut child, port) {
        cleanup(&mut child);
        return Err("A2A binary did not become reachable".into());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .no_proxy()
        .build()?;
    let base_url = format!("http://127.0.0.1:{port}");
    let outcome = async {
        let public = client
            .get(format!("{base_url}/.well-known/agent.json"))
            .send()
            .await?;
        let public_status = public.status();
        let public_card = public.json::<serde_json::Value>().await?;

        let unauthenticated = client
            .get(format!("{base_url}/private/health"))
            .send()
            .await?;
        let unauthenticated_status = unauthenticated.status();

        let authenticated = client
            .get(format!("{base_url}/private/health"))
            .bearer_auth(PAIRING_TOKEN)
            .send()
            .await?;
        let authenticated_status = authenticated.status();
        let authenticated_health = authenticated.json::<serde_json::Value>().await?;

        let invalid_task = openlife_core::a2a::A2AClient::build_text_task(
            None,
            "task without the required ContextManifest",
        );
        let invalid_task_status = client
            .post(format!("{base_url}/tasks/send"))
            .bearer_auth(PAIRING_TOKEN)
            .json(&invalid_task)
            .send()
            .await?
            .status();

        let oversized_status = client
            .post(format!("{base_url}/tasks/send"))
            .bearer_auth(PAIRING_TOKEN)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body("x".repeat(512 * 1024 + 1))
            .send()
            .await?
            .status();

        Ok::<_, reqwest::Error>((
            public_status,
            public_card,
            unauthenticated_status,
            authenticated_status,
            authenticated_health,
            invalid_task_status,
            oversized_status,
        ))
    }
    .await;
    cleanup(&mut child);

    let (
        public_status,
        public_card,
        unauthenticated_status,
        authenticated_status,
        authenticated_health,
        invalid_task_status,
        oversized_status,
    ) = outcome?;
    assert_eq!(public_status, reqwest::StatusCode::OK);
    assert_eq!(public_card["name"], "OpenLife");
    assert_eq!(public_card["skills"], serde_json::json!([]));
    assert!(public_card.get("values").is_none());
    assert!(public_card.get("goals").is_none());
    assert_eq!(unauthenticated_status, reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(authenticated_status, reqwest::StatusCode::OK);
    assert_eq!(authenticated_health["port"], port);
    assert_eq!(invalid_task_status, reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(oversized_status, reqwest::StatusCode::PAYLOAD_TOO_LARGE);
    Ok(())
}
