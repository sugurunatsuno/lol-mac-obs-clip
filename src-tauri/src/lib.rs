// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

use tauri::Manager;
mod lol_events;


pub fn start_lol_listener(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = lol_events::listen_for_events(app_handle).await {
            eprintln!("LoL listener error: {err}");
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            start_lol_listener(app.app_handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
