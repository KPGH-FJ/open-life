use tauri::Manager;

pub(crate) fn ensure_main_window_visible<R: tauri::Runtime, M: Manager<R>>(
    manager: &M,
) -> tauri::Result<()> {
    let window = if let Some(window) = manager.get_webview_window("main") {
        window
    } else {
        tauri::WebviewWindowBuilder::new(
            manager,
            "main",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("OpenLife")
        .inner_size(1280.0, 800.0)
        .resizable(true)
        .center()
        .visible(true)
        .focused(true)
        .build()?
    };

    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    Ok(())
}
