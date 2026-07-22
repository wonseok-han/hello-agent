mod agent;
mod detect;
mod editor;
mod error;
mod install;
mod login;
mod project;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(login::LoginSession::default())
        .invoke_handler(tauri::generate_handler![
            detect::detect_environment,
            install::install_agent,
            login::login_status,
            login::start_login,
            login::submit_login_code,
            login::cancel_login,
            project::create_first_project,
            project::run_first_chat,
            editor::detect_editors,
            editor::open_in_editor
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
