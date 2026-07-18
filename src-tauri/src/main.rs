#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(exit_code) =
        openlife_core::resource_gateway::run_resource_parser_worker_if_requested()
    {
        std::process::exit(exit_code);
    }
    openlife_tauri_lib::run();
}
