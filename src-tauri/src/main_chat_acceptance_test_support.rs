use crate::main_chat_command_surface_eval;
use crate::main_chat_command_surface_eval::MainChatCommandSurfaceEvalReport;
use crate::AppState;
use std::sync::Arc;

pub(crate) async fn configure_live_provider_eval_state(state: &Arc<AppState>) {
    {
        let mut config = state.config.lock().await;
        config.llm.provider = std::env::var("OPENLIFE_LIVE_EVAL_PROVIDER").unwrap_or_default();
        config.llm.openai_base = std::env::var("OPENLIFE_LIVE_EVAL_BASE").unwrap_or_default();
        config.llm.chat_model = std::env::var("OPENLIFE_LIVE_EVAL_MODEL").unwrap_or_default();
        config.llm.openai_key = std::env::var("OPENLIFE_LIVE_EVAL_API_KEY").unwrap_or_default();
        config.system.network_policy.enabled = true;
    }
    {
        let config = state.config.lock().await.clone();
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            config.local_model.clone(),
            false,
            config.llm.provider.clone(),
            config.llm.openai_base.clone(),
            config.llm.openai_key.clone(),
            config.llm.chat_model.clone(),
            config.llm.embedding_model.clone(),
            false,
        );
    }
}

pub(crate) async fn configure_live_provider_eval_state_with_local_http_provider(
    state: &Arc<AppState>,
    reply: &'static str,
) {
    let provider_base = fake_local_chat_provider_endpoint(reply).await;
    {
        let mut config = state.config.lock().await;
        config.llm.provider = "openai".into();
        config.llm.openai_base = provider_base.clone();
        config.llm.chat_model = "gpt-local-provider-harness".into();
        config.llm.openai_key = "test-key".into();
        config.system.network_policy.enabled = true;
    }
    {
        let config = state.config.lock().await.clone();
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            config.local_model.clone(),
            false,
            config.llm.provider.clone(),
            provider_base,
            config.llm.openai_key.clone(),
            config.llm.chat_model.clone(),
            config.llm.embedding_model.clone(),
            false,
        );
    }
}

async fn fake_local_chat_provider_endpoint(reply: &'static str) -> String {
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
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
                    let mut buffer = [0u8; 8192];
                    let _ = std::io::Read::read(&mut stream, &mut buffer);
                    let body = serde_json::json!({
                        "id": "chatcmpl-main-chat-live-provider-local",
                        "object": "chat.completion",
                        "choices": [{
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": reply
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

pub(crate) async fn run_main_chat_command_surface_eval_gate() -> MainChatCommandSurfaceEvalReport {
    main_chat_command_surface_eval::run_main_chat_command_surface_eval_report().await
}
