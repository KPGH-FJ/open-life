use crate::AppState;
use std::sync::Arc;

pub(crate) async fn configure_live_provider_eval_state(state: &Arc<AppState>) {
    let mut config = state.config.lock().await.clone();
    config.llm.provider = std::env::var("OPENLIFE_LIVE_EVAL_PROVIDER").unwrap_or_default();
    config.llm.openai_base = std::env::var("OPENLIFE_LIVE_EVAL_BASE").unwrap_or_default();
    config.llm.chat_model = std::env::var("OPENLIFE_LIVE_EVAL_MODEL").unwrap_or_default();
    config.llm.openai_key = std::env::var("OPENLIFE_LIVE_EVAL_API_KEY").unwrap_or_default();
    config.prefer_local_model = false;
    apply_live_search_eval_env(&mut config);
    config.system.network_policy.enabled = true;
    config.system.network_policy.default_decision = "allow".into();
    let _provider_generation = state.replace_provider_runtime_config(config).await;
}

pub(crate) fn apply_live_search_eval_env(config: &mut openlife_core::config::AppConfig) {
    let explicit_provider = std::env::var("OPENLIFE_LIVE_EVAL_SEARCH_PROVIDER")
        .ok()
        .filter(|provider| !provider.trim().is_empty());
    if let Some(provider) = explicit_provider {
        config.system.search_provider = provider;
    } else if config.llm.provider.eq_ignore_ascii_case("deepseek")
        || config.llm.provider.eq_ignore_ascii_case("openrouter")
    {
        // Exercise the product's automatic hosted-search capability on the
        // exact selected provider route. Custom gateways cannot inherit the
        // selected credential and therefore fail closed here.
        config.system.search_provider = "auto".into();
        if !config.search_reuses_selected_provider_credential() {
            config.system.search_provider = "unavailable".into();
        }
    }
    if let Ok(key) = std::env::var("OPENLIFE_LIVE_EVAL_SEARCH_API_KEY") {
        config.system.search_provider_key = key;
    }
    if let Ok(url) = std::env::var("OPENLIFE_LIVE_EVAL_SEARXNG_URL") {
        config.system.searxng_url = url;
    }
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

pub(crate) async fn configure_live_web_eval_state_with_citation_retry_local_http_provider(
    state: &Arc<AppState>,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider_base = fake_local_chat_provider_endpoint(
        "",
        Some(Arc::clone(&captured_requests)),
        LocalCitationEcho::WebAfterRetry,
    )
    .await;
    configure_local_http_provider(state, provider_base).await;
    captured_requests
}

pub(crate) async fn configure_live_web_artifact_eval_state_with_citation_echo_local_http_provider(
    state: &Arc<AppState>,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider_base = fake_local_chat_provider_endpoint(
        "",
        Some(Arc::clone(&captured_requests)),
        LocalCitationEcho::WebArtifact,
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

pub(crate) async fn configure_live_resource_and_web_artifact_eval_state_with_citation_retry_local_http_provider(
    state: &Arc<AppState>,
) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider_base = fake_local_chat_provider_endpoint(
        "",
        Some(Arc::clone(&captured_requests)),
        LocalCitationEcho::ResourceAndWebArtifactAfterRetry,
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
    WebAfterRetry,
    WebArtifact,
    AllResources,
    ResourceAndWebArtifactAfterRetry,
}

impl LocalCitationEcho {
    fn response_content(self, request_text: &str, reply: &str) -> String {
        let is_resource_citation_suffix = |suffix: &str| {
            suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                || suffix.bytes().all(|byte| (b'a'..=b'p').contains(&byte))
        };
        let issued_citation = |prefix: &str, length: usize| {
            let search_text = if prefix == "cite_" {
                request_text
                    .rsplit_once("[TRUSTED OPENLIFE FINAL OUTPUT CHECK")
                    .map(|(_, tail)| tail)
                    .unwrap_or(request_text)
            } else {
                request_text
            };
            search_text.match_indices(prefix).find_map(|(start, _)| {
                let candidate = search_text.get(start..start.checked_add(length)?)?;
                let suffix = &candidate[prefix.len()..];
                let valid = if prefix == "cite_" {
                    is_resource_citation_suffix(suffix)
                } else {
                    suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
                };
                valid.then(|| candidate.to_string())
            })
        };
        let final_answer = |content: String, web: Option<String>| {
            let source_blocks = web
                .as_ref()
                .map(|source_ref| {
                    content
                        .split("\n\n")
                        .map(str::trim)
                        .filter(|block| !block.is_empty())
                        .map(|block| {
                            if block.starts_with('#') {
                                serde_json::json!({
                                    "kind": "heading",
                                    "text": block,
                                    "sourceRefs": [],
                                })
                            } else {
                                serde_json::json!({
                                    "kind": "claim",
                                    "text": block,
                                    "sourceRefs": [source_ref.clone()],
                                })
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            serde_json::json!({
                "schemaVersion": "openlife.agent-step.v1",
                "step": {
                    "kind": "final_answer",
                    "payload": {
                        "content": if web.is_some() { String::new() } else { content },
                        "evidenceRefs": [],
                        "artifactRefs": [],
                        "sourceBlocks": source_blocks,
                    }
                }
            })
            .to_string()
        };
        let artifact = |name: &str,
                        content: String,
                        web: Option<String>,
                        review_before_write: bool| {
            let source_blocks = web
                .as_ref()
                .map(|source_ref| {
                    content
                        .split("\n\n")
                        .map(str::trim)
                        .filter(|block| !block.is_empty())
                        .map(|block| {
                            if block.starts_with('#') {
                                serde_json::json!({
                                    "kind": "heading",
                                    "text": block,
                                    "sourceRefs": [],
                                })
                            } else {
                                serde_json::json!({
                                    "kind": "claim",
                                    "text": block,
                                    "sourceRefs": [source_ref.clone()],
                                })
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            serde_json::json!({
                    "schemaVersion": "openlife.agent-step.v1",
                    "step": {
                        "kind": "draft_artifact",
                        "payload": {
                            "artifacts": [{
                                "format": "markdown",
                                "suggestedName": name,
                                "content": if web.is_some() { serde_json::Value::Null } else { serde_json::Value::String(content) },
                                "sourceBlocks": source_blocks,
                            }],
                            "reviewBeforeWrite": review_before_write,
                        }
                    }
                })
                .to_string()
        };
        let content = match self {
            Self::None => reply.to_string(),
            Self::Web => issued_citation("webref_", 31)
                .map(|citation| {
                    final_answer(
                        "The retrieved Web evidence is available.".into(),
                        Some(citation),
                    )
                })
                .unwrap_or_else(|| "No issued Web citation was observed.".into()),
            Self::WebAfterRetry => {
                if request_text.contains("TRUSTED OPENLIFE ONE-SHOT SOURCE-BINDING RETRY")
                {
                    issued_citation("webref_", 31)
                        .map(|citation| {
                            final_answer(
                                "The retrieved Web evidence is available.".into(),
                                Some(citation),
                            )
                        })
                        .unwrap_or_else(|| "No issued Web citation was observed.".into())
                } else {
                    final_answer("The first draft intentionally omitted its citation.".into(), None)
                }
            }
            Self::WebArtifact => issued_citation("webref_", 31)
                .map(|citation| {
                    if request_text.contains("TRUSTED OPENLIFE ONE-SHOT SOURCE-BINDING RETRY") {
                        artifact(
                            "continuous-learning.md",
                            "# Continuous learning\n\n现有公开网页证据只支持页面明确陈述的受限结论；无法据此作更广泛的产品外推。".into(),
                            Some(citation),
                            false,
                        )
                    } else {
                        artifact(
                            "continuous-learning.md",
                            "# Continuous learning\n\n页面标题暗示该结论可以无条件推广到其他产品。".into(),
                            None,
                            false,
                        )
                    }
                })
                .unwrap_or_else(|| {
                    serde_json::json!({
                        "schemaVersion": "openlife.agent-step.v1",
                        "step": {
                            "kind": "draft_artifact",
                            "payload": {
                                "artifacts": [{
                                    "format": "markdown",
                                    "suggestedName": "continuous-learning.md",
                                    "content": "Provider did not observe an issued Web citation."
                                }],
                                "reviewBeforeWrite": false
                            }
                        }
                    })
                    .to_string()
                }),
            Self::AllResources => {
                let resource_contract = request_text
                    .rsplit_once("[TRUSTED OPENLIFE FINAL OUTPUT CHECK")
                    .map(|(_, tail)| tail)
                    .unwrap_or(request_text);
                let citations = resource_contract
                    .match_indices("cite_")
                    .filter_map(|(start, _)| {
                        let candidate = resource_contract.get(start..start.checked_add(29)?)?;
                        is_resource_citation_suffix(&candidate[5..]).then(|| candidate.to_string())
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                if citations.is_empty() {
                    "No issued Resource citation was observed.".into()
                } else {
                    let source_refs = citations.into_iter().collect::<Vec<_>>();
                    let content =
                        "The bounded comparison and analysis used every selected Resource.";
                    serde_json::json!({
                        "schemaVersion": "openlife.agent-step.v1",
                        "step": {
                            "kind": "final_answer",
                            "payload": {
                                "content": "",
                                "evidenceRefs": [],
                                "artifactRefs": [],
                                "sourceBlocks": [{
                                    "kind": "claim",
                                    "text": content,
                                    "sourceRefs": source_refs
                                }]
                            }
                        }
                    })
                    .to_string()
                }
            }
            Self::ResourceAndWebArtifactAfterRetry => {
                let resource = issued_citation("cite_", 29);
                let web = issued_citation("webref_", 31);
                let repair = request_text
                    .contains("TRUSTED OPENLIFE ONE-SHOT SOURCE-BINDING RETRY");
                let content = match (resource, web, repair) {
                    (Some(resource), Some(web), true) => serde_json::json!({
                        "schemaVersion": "openlife.agent-step.v1",
                        "step": {
                            "kind": "draft_artifact",
                            "payload": {
                                "artifacts": [{
                                    "format": "markdown",
                                    "suggestedName": "evidence-report.md",
                                    "content": null,
                                    "sourceBlocks": [
                                        {"kind": "heading", "text": "# 带引用的报告", "sourceRefs": []},
                                        {"kind": "claim", "text": "附件证据已纳入。", "sourceRefs": [resource]},
                                        {"kind": "claim", "text": "公开网页证据已纳入。", "sourceRefs": [web]}
                                    ]
                                }],
                                "reviewBeforeWrite": true
                            }
                        }
                    }).to_string(),
                    (_, Some(web), false) => artifact(
                        "evidence-report.md",
                        "# 缺少附件引用的首轮草稿\n\n公开网页证据已纳入。".into(),
                        Some(web),
                        true,
                    ),
                    _ => artifact(
                        "evidence-report.md",
                        "Provider did not observe the issued citation classes.".into(),
                        None,
                        true,
                    ),
                };
                content
            }
        };
        if request_text.contains("final_answer")
            && serde_json::from_str::<openlife_core::work_orchestration::AgentStepEnvelope>(
                &content,
            )
            .is_err()
        {
            serde_json::json!({
                "schemaVersion": "openlife.agent-step.v1",
                "step": {
                    "kind": "final_answer",
                    "payload": {
                        "content": content,
                        "evidenceRefs": [],
                        "artifactRefs": []
                    }
                }
            })
            .to_string()
        } else {
            content
        }
    }
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

pub(crate) async fn grant_canonical_web_search_once(state: &Arc<AppState>) {
    state
        .tool_permission_store
        .lock()
        .await
        .grant(
            "web.search",
            "builtin",
            "medium",
            "read",
            openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
            None,
        )
        .expect("grant explicit one-shot web.search permission");
}

pub(crate) fn isolated_canonical_state_with_resource_runtime() -> Arc<AppState> {
    let store = openlife_core::resource::ResourceStore::new_in_memory()
        .expect("create isolated canonical resource store");
    let runtime = crate::resource_commands::ResourceRuntime::new(
        openlife_core::resource_gateway::ResourceGateway::new(
            store,
            openlife_core::resource_gateway::ResourceParserProcess::for_current_executable()
                .expect("resource parser process"),
        ),
    );
    let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    Arc::get_mut(&mut state)
        .expect("isolated canonical state must have one owner")
        .resource_runtime = Some(Arc::new(runtime));
    state
}

pub(crate) fn import_frozen_resources_to_canonical_state(
    state: &Arc<AppState>,
    operation_id: &str,
    sources: Vec<openlife_core::resource_gateway::ResourceImportSource>,
) {
    let expected_count = sources.len();
    let resources = sources
        .into_iter()
        .map(|source| {
            let extraction = openlife_core::resource_parser::extract_resource(
                openlife_core::resource_parser::ResourceExtractionRequest {
                    filename: source.filename.clone(),
                    declared_mime: source.declared_mime.clone(),
                    bytes: source.bytes.clone(),
                },
            )
            .expect("extract frozen resource with the production bounded parser");
            openlife_core::resource::ResourceImportCandidate {
                resource_id: uuid::Uuid::new_v4().to_string(),
                filename: source.filename,
                declared_mime: source.declared_mime,
                detected_mime: extraction.detected_mime,
                format: extraction.format,
                bytes: source.bytes,
                chunks: extraction.chunks,
            }
        })
        .collect();
    let receipt = state
        .resource_runtime
        .as_ref()
        .expect("canonical resource runtime")
        .gateway()
        .store()
        .commit_import_batch(openlife_core::resource::ResourceImportBatch {
            operation_id: uuid::Uuid::new_v4().to_string(),
            message_id: operation_id.to_string(),
            resources,
        })
        .expect("bind production-parsed frozen resources to ResourceStore");
    assert_eq!(receipt.resources.len(), expected_count);
}
