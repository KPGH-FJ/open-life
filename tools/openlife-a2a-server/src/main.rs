use openlife_tauri_lib::a2a_server::{
    build_a2a_router, configured_a2a_port, load_persisted_a2a_runtime_state,
    require_authenticated_dev_a2a_opt_in, A2A_PARENT_PIPE_GUARD_ENV,
};
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() {
    let pairing_token = match require_authenticated_dev_a2a_opt_in() {
        Ok(token) => token,
        Err(reason) => {
            eprintln!("[A2A] development server refused: {reason}");
            std::process::exit(2);
        }
    };
    let port = std::env::var("A2A_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(configured_a2a_port);
    let runtime_state = match load_persisted_a2a_runtime_state(port) {
        Ok(state) => state,
        Err(reason) => {
            eprintln!("[A2A] runtime state refused: {reason}");
            std::process::exit(2);
        }
    };
    let app = build_a2a_router(runtime_state, pairing_token);

    let bind_addr = format!("127.0.0.1:{port}");
    match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => {
            let addr = listener
                .local_addr()
                .map(|address| address.to_string())
                .unwrap_or_else(|_| bind_addr.clone());
            println!("[A2A] paired HTTP server listening on http://{addr}");
            let server = axum::serve(listener, app);
            if std::env::var(A2A_PARENT_PIPE_GUARD_ENV).as_deref() == Ok("1") {
                tokio::select! {
                    result = server => {
                        if let Err(error) = result {
                            eprintln!("[A2A] server error: {error}");
                        }
                    }
                    _ = wait_for_parent_pipe_close() => {
                        println!("[A2A] parent pipe closed; stopping development sidecar");
                    }
                }
            } else if let Err(error) = server.await {
                eprintln!("[A2A] server error: {error}");
            }
        }
        Err(error) => eprintln!("[A2A] failed to bind server: {error}"),
    }
}

async fn wait_for_parent_pipe_close() {
    let mut parent_pipe = tokio::io::stdin();
    let mut buffer = [0_u8; 64];
    loop {
        match parent_pipe.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
    }
}
