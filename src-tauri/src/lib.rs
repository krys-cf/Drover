use tauri_plugin_frame::FramePluginBuilder;

mod ai;
mod files;
mod mcp;
mod shell;
mod tools;

use portable_pty::MasterPty;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

pub(crate) struct PtySession {
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn MasterPty + Send>,
    pub child_pid: Option<u32>,
}

pub(crate) struct AppState {
    pub sessions: Mutex<HashMap<String, PtySession>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct TerminalOutput {
    pub session_id: String,
    pub data: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(AppState {
        sessions: Mutex::new(HashMap::new()),
    });

    tauri::Builder::default()
        .manage(state)
        .plugin(
            FramePluginBuilder::new()
                .titlebar_height(44)
                .button_width(46)
                .auto_titlebar(true)
                .button_hover_bg("rgba(255,255,255,0.08)")
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            use tauri::Manager;
            let salt_path = app
                .path()
                .app_local_data_dir()
                .expect("could not resolve app local data path")
                .join("salt.txt");
            app.handle()
                .plugin(tauri_plugin_stronghold::Builder::with_argon2(&salt_path).build())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            shell::spawn_shell,
            shell::write_to_shell,
            shell::resize_pty,
            shell::get_shell_cwd,
            ai::ai_chat,
            ai::ai_summarize,
            files::list_directory,
            files::read_file_contents,
            files::write_file_contents,
            files::list_remote_directory,
            files::read_remote_file_contents,
            files::write_remote_file_contents,
            files::read_file_base64,
            files::read_remote_file_base64,
            files::create_file,
            files::create_directory,
            files::rename_path,
            files::delete_path,
            files::copy_path,
            files::select_files_dialog,
            files::read_file_text,
            files::search_files,
            shell::detect_ssh,
            tools::dig_domain,
            tools::dig_trace,
            tools::run_curl,
            tools::openssl_check_cert,
            tools::openssl_decode_cert,
            tools::openssl_hash,
            tools::run_whois,
            tools::run_ping,
            tools::run_traceroute,
            tools::run_port_scan,
            mcp::mcp_list_servers,
            mcp::mcp_save_servers,
            mcp::mcp_initialize,
            mcp::mcp_list_tools,
            mcp::mcp_call_tool,
            ai::ai_chat_with_tools,
            ai::ai_list_sessions,
            ai::ai_create_session,
            ai::ai_summarize_tool_results,
            ai::ai_retry,
            ai::ai_edit_code,
            ai::ai_edit_code_from_path,
            ai::ai_gemini_chat,
            ai::ai_gemini_edit_code,
            ai::ai_gemini_edit_code_from_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
