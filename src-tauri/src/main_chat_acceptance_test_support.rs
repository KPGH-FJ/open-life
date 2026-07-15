use crate::main_chat_command_surface_eval;
use crate::main_chat_command_surface_eval::MainChatCommandSurfaceEvalReport;
use crate::AppState;
use std::sync::Arc;

pub(crate) async fn configure_live_provider_eval_state(state: &Arc<AppState>) {
    let mut config = state.config.lock().await.clone();
    config.llm.provider = std::env::var("OPENLIFE_LIVE_EVAL_PROVIDER").unwrap_or_default();
    config.llm.openai_base = std::env::var("OPENLIFE_LIVE_EVAL_BASE").unwrap_or_default();
    config.llm.chat_model = std::env::var("OPENLIFE_LIVE_EVAL_MODEL").unwrap_or_default();
    config.llm.openai_key = std::env::var("OPENLIFE_LIVE_EVAL_API_KEY").unwrap_or_default();
    config.prefer_local_model = false;
    config.system.network_policy.enabled = true;
    config.system.network_policy.default_decision = "allow".into();
    let _provider_generation = state.replace_provider_runtime_config(config).await;
}

pub(crate) async fn configure_live_provider_eval_state_with_local_http_provider(
    state: &Arc<AppState>,
    reply: &'static str,
) {
    let provider_base =
        fake_local_chat_provider_endpoint(reply, None, LocalCitationEcho::None).await;
    configure_local_http_provider(state, provider_base).await;
}

pub(crate) async fn configure_live_provider_eval_state_with_captured_local_http_provider(
    state: &Arc<AppState>,
    reply: &'static str,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider_base = fake_local_chat_provider_endpoint(
        reply,
        Some(Arc::clone(&captured_requests)),
        LocalCitationEcho::None,
    )
    .await;
    configure_local_http_provider(state, provider_base).await;
    captured_requests
}

pub(crate) async fn configure_live_web_eval_state_with_citation_echo_local_http_provider(
    state: &Arc<AppState>,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider_base = fake_local_chat_provider_endpoint(
        "",
        Some(Arc::clone(&captured_requests)),
        LocalCitationEcho::Web,
    )
    .await;
    configure_local_http_provider(state, provider_base).await;
    captured_requests
}

pub(crate) async fn configure_live_resource_and_web_eval_state_with_citation_echo_local_http_provider(
    state: &Arc<AppState>,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider_base = fake_local_chat_provider_endpoint(
        "",
        Some(Arc::clone(&captured_requests)),
        LocalCitationEcho::ResourceAndWeb,
    )
    .await;
    configure_local_http_provider(state, provider_base).await;
    captured_requests
}

pub(crate) async fn configure_live_resource_eval_state_with_all_citations_local_http_provider(
    state: &Arc<AppState>,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider_base = fake_local_chat_provider_endpoint(
        "",
        Some(Arc::clone(&captured_requests)),
        LocalCitationEcho::AllResources,
    )
    .await;
    configure_local_http_provider(state, provider_base).await;
    captured_requests
}

pub(crate) async fn configure_live_resource_and_web_artifact_eval_state_with_citation_echo_local_http_provider(
    state: &Arc<AppState>,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider_base = fake_local_chat_provider_endpoint(
        "",
        Some(Arc::clone(&captured_requests)),
        LocalCitationEcho::ResourceAndWebArtifact,
    )
    .await;
    configure_local_http_provider(state, provider_base).await;
    captured_requests
}

pub(crate) async fn configure_live_resource_and_forged_web_artifact_eval_state_with_local_http_provider(
    state: &Arc<AppState>,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider_base = fake_local_chat_provider_endpoint(
        "",
        Some(Arc::clone(&captured_requests)),
        LocalCitationEcho::ResourceAndForgedWebArtifact,
    )
    .await;
    configure_local_http_provider(state, provider_base).await;
    captured_requests
}

pub(crate) async fn configure_live_provider_eval_state_with_barriered_streaming_local_http_provider(
    state: &Arc<AppState>,
    chunks: Vec<(&'static str, std::time::Duration)>,
) -> Arc<std::sync::atomic::AtomicBool> {
    let release_remaining_chunks = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let provider_base = fake_streaming_local_chat_provider_endpoint(
        chunks,
        Arc::clone(&release_remaining_chunks),
        None,
    )
    .await;
    configure_local_http_provider(state, provider_base).await;
    release_remaining_chunks
}

pub(crate) async fn configure_live_provider_eval_state_with_captured_streaming_local_http_provider(
    state: &Arc<AppState>,
    chunks: Vec<(&'static str, std::time::Duration)>,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    use std::sync::atomic::AtomicBool;

    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let release_remaining_chunks = Arc::new(AtomicBool::new(true));
    let provider_base = fake_streaming_local_chat_provider_endpoint(
        chunks,
        Arc::clone(&release_remaining_chunks),
        Some(Arc::clone(&captured_requests)),
    )
    .await;
    configure_local_http_provider(state, provider_base).await;
    captured_requests
}

pub(crate) async fn configure_live_provider_eval_state_with_failing_local_http_provider(
    state: &Arc<AppState>,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider_base =
        fake_failing_local_chat_provider_endpoint(Arc::clone(&captured_requests)).await;
    configure_local_http_provider(state, provider_base).await;
    captured_requests
}

pub(crate) async fn configure_live_provider_eval_state_with_hanging_local_http_provider(
    state: &Arc<AppState>,
) -> (
    Arc<std::sync::atomic::AtomicBool>,
    Arc<std::sync::atomic::AtomicBool>,
    Arc<std::sync::atomic::AtomicBool>,
    Arc<std::sync::atomic::AtomicBool>,
) {
    let (
        provider_base,
        request_observed,
        client_closed,
        release_late_response,
        late_response_attempted,
    ) = fake_hanging_local_chat_provider_endpoint().await;
    configure_local_http_provider(state, provider_base).await;
    (
        request_observed,
        client_closed,
        release_late_response,
        late_response_attempted,
    )
}

async fn configure_local_http_provider(state: &Arc<AppState>, provider_base: String) {
    let mut config = state.config.lock().await.clone();
    config.llm.provider = "openai".into();
    config.llm.openai_base = provider_base;
    config.llm.chat_model = "gpt-local-provider-harness".into();
    config.llm.openai_key = "test-key".into();
    config.prefer_local_model = false;
    config.system.network_policy.enabled = true;
    config.system.network_policy.default_decision = "allow".into();
    let _provider_generation = state.replace_provider_runtime_config(config).await;
}

async fn fake_local_chat_provider_endpoint(
    reply: &'static str,
    captured_requests: Option<Arc<std::sync::Mutex<Vec<String>>>>,
    citation_echo: LocalCitationEcho,
) -> String {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind local fake chat provider");
    let addr = listener.local_addr().expect("local fake provider addr");
    std::thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut handled = 0usize;
        while handled < 8 && std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    handled += 1;
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
                    let mut request_bytes = Vec::new();
                    let mut buffer = [0u8; 8192];
                    loop {
                        match std::io::Read::read(&mut stream, &mut buffer) {
                            Ok(0) => break,
                            Ok(read) => {
                                request_bytes.extend_from_slice(&buffer[..read]);
                                let request = String::from_utf8_lossy(&request_bytes);
                                let complete = request.find("\r\n\r\n").is_some_and(|header_end| {
                                    let content_length = request[..header_end]
                                        .lines()
                                        .find_map(|line| {
                                            let (name, value) = line.split_once(':')?;
                                            name.eq_ignore_ascii_case("content-length")
                                                .then(|| value.trim().parse::<usize>().ok())
                                                .flatten()
                                        })
                                        .unwrap_or(0);
                                    request_bytes.len() >= header_end + 4 + content_length
                                });
                                if complete {
                                    break;
                                }
                            }
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                                ) =>
                            {
                                break;
                            }
                            Err(_) => break,
                        }
                    }
                    let request_text = String::from_utf8_lossy(&request_bytes).into_owned();
                    if let Some(captured_requests) = captured_requests.as_ref() {
                        captured_requests
                            .lock()
                            .expect("capture local provider request")
                            .push(request_text.clone());
                    }
                    let response_content = citation_echo.response_content(&request_text, reply);
                    let body = serde_json::json!({
                        "id": "chatcmpl-main-chat-live-provider-local",
                        "object": "chat.completion",
                        "choices": [{
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": response_content
                            },
                            "finish_reason": "stop"
                        }]
                    })
                    .to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    format!("http://{addr}/v1")
}

#[derive(Clone, Copy)]
enum LocalCitationEcho {
    None,
    Web,
    AllResources,
    ResourceAndWeb,
    ResourceAndWebArtifact,
    ResourceAndForgedWebArtifact,
}

impl LocalCitationEcho {
    fn response_content(self, request_text: &str, reply: &str) -> String {
        let issued_citation = |prefix: &str, length: usize| {
            request_text.match_indices(prefix).find_map(|(start, _)| {
                let candidate = request_text.get(start..start.checked_add(length)?)?;
                candidate[prefix.len()..]
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
                    .then(|| candidate.to_string())
            })
        };
        match self {
            Self::None => reply.to_string(),
            Self::Web => issued_citation("webref_", 31)
                .map(|citation| format!("The retrieved Web evidence is available [{citation}]."))
                .unwrap_or_else(|| "No issued Web citation was observed.".into()),
            Self::AllResources => {
                let citations = request_text
                    .match_indices("cite_")
                    .filter_map(|(start, _)| {
                        let candidate = request_text.get(start..start.checked_add(29)?)?;
                        candidate[5..]
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit())
                            .then(|| candidate.to_string())
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                if citations.is_empty() {
                    "No issued Resource citation was observed.".into()
                } else {
                    format!(
                        "The bounded comparison and analysis used every selected Resource citation: {}.",
                        citations
                            .into_iter()
                            .map(|citation| format!("[{citation}]"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                }
            }
            Self::ResourceAndWeb => {
                let resource = issued_citation("cite_", 29);
                let web = issued_citation("webref_", 31);
                match (resource, web) {
                    (Some(resource), Some(web)) => format!(
                        "Synthesis used the issued Resource citation [{resource}] and the issued Web citation [{web}]."
                    ),
                    _ => "Both issued Resource and Web citations were not observed.".into(),
                }
            }
            Self::ResourceAndWebArtifact => {
                let resource = issued_citation("cite_", 29);
                let web = issued_citation("webref_", 31);
                match (resource, web) {
                    (Some(resource), Some(web)) => serde_json::json!({
                        "markdown": format!(
                            "# 带引用的路演报告\n\n附件证据 [{resource}] 与公开网页证据 [{web}] 已共同纳入风险分析。"
                        )
                    })
                    .to_string(),
                    _ => serde_json::json!({
                        "markdown": "Provider did not observe both issued citation classes."
                    })
                    .to_string(),
                }
            }
            Self::ResourceAndForgedWebArtifact => {
                let resource = issued_citation("cite_", 29)
                    .unwrap_or_else(|| "cite_aaaaaaaaaaaaaaaaaaaaaaaa".into());
                serde_json::json!({
                    "markdown": format!(
                        "# Forged citation report\n\nValid Resource [{resource}], forged Web [webref_aaaaaaaaaaaaaaaaaaaaaaaa]."
                    )
                })
                .to_string()
            }
        }
    }
}

async fn fake_streaming_local_chat_provider_endpoint(
    chunks: Vec<(&'static str, std::time::Duration)>,
    release_remaining_chunks: Arc<std::sync::atomic::AtomicBool>,
    captured_requests: Option<Arc<std::sync::Mutex<Vec<String>>>>,
) -> String {
    use std::sync::atomic::Ordering;

    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind local streaming chat provider");
    let addr = listener
        .local_addr()
        .expect("local streaming provider addr");
    std::thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("accept streaming provider request");
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
        let mut request_bytes = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            match std::io::Read::read(&mut stream, &mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    request_bytes.extend_from_slice(&buffer[..read]);
                    let request = String::from_utf8_lossy(&request_bytes);
                    let complete = request.find("\r\n\r\n").is_some_and(|header_end| {
                        let content_length = request[..header_end]
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().ok())
                                    .flatten()
                            })
                            .unwrap_or(0);
                        request_bytes.len() >= header_end + 4 + content_length
                    });
                    if complete {
                        break;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(_) => break,
            }
        }
        if let Some(captured_requests) = captured_requests.as_ref() {
            captured_requests
                .lock()
                .expect("capture local streaming provider request")
                .push(String::from_utf8_lossy(&request_bytes).into_owned());
        }

        let body = chunks
            .iter()
            .map(|(chunk, _)| {
                format!(
                    "data: {}\n\n",
                    serde_json::json!({
                        "id": "chatcmpl-main-chat-streaming-provider",
                        "object": "chat.completion.chunk",
                        "choices": [{
                            "index": 0,
                            "delta": { "content": chunk },
                            "finish_reason": serde_json::Value::Null
                        }]
                    })
                )
            })
            .collect::<String>()
            + "data: [DONE]\n\n";
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        std::io::Write::write_all(&mut stream, header.as_bytes())
            .expect("write streaming provider headers");
        std::io::Write::flush(&mut stream).expect("flush streaming provider headers");
        for (index, (chunk, delay)) in chunks.into_iter().enumerate() {
            std::thread::sleep(delay);
            let event = format!(
                "data: {}\n\n",
                serde_json::json!({
                    "id": "chatcmpl-main-chat-streaming-provider",
                    "object": "chat.completion.chunk",
                    "choices": [{
                        "index": 0,
                        "delta": { "content": chunk },
                        "finish_reason": serde_json::Value::Null
                    }]
                })
            );
            std::io::Write::write_all(&mut stream, event.as_bytes())
                .expect("write streaming provider chunk");
            std::io::Write::flush(&mut stream).expect("flush streaming provider chunk");
            if index == 0 {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while !release_remaining_chunks.load(Ordering::SeqCst)
                    && std::time::Instant::now() < deadline
                {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }
        std::io::Write::write_all(&mut stream, b"data: [DONE]\n\n")
            .expect("write streaming provider completion");
        std::io::Write::flush(&mut stream).expect("flush streaming provider completion");
    });
    format!("http://{addr}/v1")
}

async fn fake_failing_local_chat_provider_endpoint(
    captured_requests: Arc<std::sync::Mutex<Vec<String>>>,
) -> String {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind local failing chat provider");
    let addr = listener.local_addr().expect("local failing provider addr");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept failing provider request");
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
        let mut request_bytes = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            match std::io::Read::read(&mut stream, &mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    request_bytes.extend_from_slice(&buffer[..read]);
                    let request = String::from_utf8_lossy(&request_bytes);
                    let complete = request.find("\r\n\r\n").is_some_and(|header_end| {
                        let content_length = request[..header_end]
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().ok())
                                    .flatten()
                            })
                            .unwrap_or(0);
                        request_bytes.len() >= header_end + 4 + content_length
                    });
                    if complete {
                        break;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(_) => break,
            }
        }
        captured_requests
            .lock()
            .expect("capture failing local provider request")
            .push(String::from_utf8_lossy(&request_bytes).into_owned());
        let body = serde_json::json!({
            "error": {
                "type": "roadshow_provider_unavailable",
                "message": "injected provider failure"
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        let _ = std::io::Write::flush(&mut stream);
    });
    format!("http://{addr}/v1")
}

async fn fake_hanging_local_chat_provider_endpoint() -> (
    String,
    Arc<std::sync::atomic::AtomicBool>,
    Arc<std::sync::atomic::AtomicBool>,
    Arc<std::sync::atomic::AtomicBool>,
    Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;

    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind local hanging chat provider");
    let addr = listener.local_addr().expect("local hanging provider addr");
    let request_observed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let client_closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let release_late_response = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let late_response_attempted = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_for_thread = Arc::clone(&request_observed);
    let closed_for_thread = Arc::clone(&client_closed);
    let release_for_thread = Arc::clone(&release_late_response);
    let attempted_for_thread = Arc::clone(&late_response_attempted);
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept hanging provider request");
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(25)));
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(1)));
        let mut request_bytes = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            match std::io::Read::read(&mut stream, &mut buffer) {
                Ok(0) => {
                    closed_for_thread.store(true, Ordering::SeqCst);
                    return;
                }
                Ok(read) => {
                    request_bytes.extend_from_slice(&buffer[..read]);
                    let request = String::from_utf8_lossy(&request_bytes);
                    let complete = request.find("\r\n\r\n").is_some_and(|header_end| {
                        let content_length = request[..header_end]
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().ok())
                                    .flatten()
                            })
                            .unwrap_or(0);
                        request_bytes.len() >= header_end + 4 + content_length
                    });
                    if complete {
                        observed_for_thread.store(true, Ordering::SeqCst);
                        break;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => return,
            }
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !release_for_thread.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            match std::io::Read::read(&mut stream, &mut buffer) {
                Ok(0) => {
                    closed_for_thread.store(true, Ordering::SeqCst);
                }
                Ok(_) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => {
                    closed_for_thread.store(true, Ordering::SeqCst);
                }
            }
        }
        let body = serde_json::json!({
            "id": "chatcmpl-main-chat-late-provider",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "late provider response" },
                "finish_reason": "stop"
            }]
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        attempted_for_thread.store(true, Ordering::SeqCst);
        let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        let _ = std::io::Write::flush(&mut stream);
    });
    (
        format!("http://{addr}/v1"),
        request_observed,
        client_closed,
        release_late_response,
        late_response_attempted,
    )
}

pub(crate) async fn run_main_chat_command_surface_eval_gate() -> MainChatCommandSurfaceEvalReport {
    main_chat_command_surface_eval::run_main_chat_command_surface_eval_report().await
}
