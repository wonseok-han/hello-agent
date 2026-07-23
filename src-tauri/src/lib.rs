mod agent;
mod detect;
mod editor;
mod error;
mod install;
mod login;
mod project;
mod status;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
            project::scan_projects,
            project::default_projects_dir,
            editor::detect_editors,
            editor::open_in_editor,
            status::agent_status,
            status::latest_agent_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
