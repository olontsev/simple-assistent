mod args;
mod commands;
mod config;
mod models;
mod server;
mod tray;

use commands::AppState;
use server::SharedServer;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tray::TrayState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_settings_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .manage(Mutex::new(server::ServerManager::default()) as SharedServer)
        .manage(Mutex::new(TrayState::default()))
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app_data_dir: {e}"))?;
            std::fs::create_dir_all(&app_data_dir)?;

            let cfg = config::load_config(&app_data_dir)?;
            let models = commands::initial_models(&cfg);

            // Sync OS autostart with saved preference
            {
                use tauri_plugin_autostart::ManagerExt;
                let launcher = app.autolaunch();
                if cfg.autostart {
                    let _ = launcher.enable();
                } else {
                    let _ = launcher.disable();
                }
            }

            app.manage(AppState {
                config: Mutex::new(cfg.clone()),
                models: Mutex::new(models.clone()),
                app_data_dir,
            });

            // Hide main window on close instead of quitting
            if let Some(win) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        tray::hide_settings_window(&handle);
                    }
                });
            }

            let status = server::ServerStatus::default();
            tray::rebuild_tray(app.handle(), &cfg, &models, &status)?;

            // Health / process poll loop
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    commands::poll_server(&handle);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_settings,
            commands::upsert_profile,
            commands::delete_profile,
            commands::set_active_profile,
            commands::set_active_model,
            commands::list_models,
            commands::get_status,
            commands::start_server,
            commands::stop_server,
            commands::load_model,
            commands::unload_model,
            commands::show_settings,
            commands::hide_settings,
            commands::get_autostart_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
