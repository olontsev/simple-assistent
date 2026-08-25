use crate::args::validate_profile_args;
use crate::config::{self, AppConfig, Profile};
use crate::models::{self, ModelEntry};
use crate::server::{ServerPhase, ServerStatus, SharedServer};
use crate::tray;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub models: Mutex<Vec<ModelEntry>>,
    pub app_data_dir: PathBuf,
}

fn refresh_models(state: &AppState) -> Result<Vec<ModelEntry>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let list = if config.models_dir.trim().is_empty() {
        Vec::new()
    } else {
        models::scan_models(&config.models_dir).unwrap_or_else(|_| Vec::new())
    };
    drop(config);
    *state.models.lock().map_err(|e| e.to_string())? = list.clone();
    Ok(list)
}

pub fn refresh_ui(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let server = app.state::<SharedServer>();
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let models = state.models.lock().map_err(|e| e.to_string())?.clone();
    let status = server.lock().map_err(|e| e.to_string())?.status();

    tray::rebuild_tray(app, &config, &models, &status).map_err(|e| e.to_string())?;
    tray::emit_status(app, &status);
    Ok(())
}

/// Rescan .gguf files and rebuild the tray menu (call before menu is shown).
pub fn refresh_models_for_menu(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let models = refresh_models(&state)?;
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let status = app
        .state::<SharedServer>()
        .lock()
        .map_err(|e| e.to_string())?
        .status();

    tray::rebuild_tray(app, &config, &models, &status).map_err(|e| e.to_string())?;
    tray::emit_models(app, &models);
    Ok(())
}

pub fn handle_tray_menu(app: &AppHandle, id: &str) {
    let result = match id {
        "start" => cmd_start_server(app.clone()),
        "stop" => cmd_stop_server(app.clone()),
        "load" => cmd_load_model(app.clone()),
        "unload" => cmd_unload_model(app.clone()),
        "settings" => {
            tray::show_settings_window(app);
            Ok(())
        }
        "quit" => {
            let server = app.state::<SharedServer>();
            if let Ok(mut s) = server.lock() {
                let _ = s.stop();
            }
            app.exit(0);
            Ok(())
        }
        other if other.starts_with("model:") => {
            let path = other.strip_prefix("model:").unwrap_or("").to_string();
            set_active_model_inner(app, Some(path))
        }
        other if other.starts_with("profile:") => {
            let pid = other.strip_prefix("profile:").unwrap_or("").to_string();
            set_active_profile_inner(app, pid)
        }
        _ => Ok(()),
    };
    if let Err(e) = &result {
        if let Ok(mut s) = app.state::<SharedServer>().lock() {
            s.set_last_error(e.clone());
        }
        let _ = refresh_ui(app);
    }
}

fn set_active_model_inner(app: &AppHandle, path: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        config.active_model_path = path;
        config::save_config(&state.app_data_dir, &config)?;
    }
    refresh_ui(app)
}

fn set_active_profile_inner(app: &AppHandle, profile_id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        if !config.profiles.iter().any(|p| p.id == profile_id) {
            return Err("Profile not found".into());
        }
        config.active_profile_id = profile_id;
        config::save_config(&state.app_data_dir, &config)?;
    }
    refresh_ui(app)
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    llama_cpp_path: String,
    models_dir: String,
    autostart: bool,
) -> Result<AppConfig, String> {
    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        config.llama_cpp_path = llama_cpp_path;
        config.models_dir = models_dir;
        config.autostart = autostart;
        config::save_config(&state.app_data_dir, &config)?;
    }

    let autostart_manager = app.autolaunch();
    if autostart {
        let _ = autostart_manager.enable();
    } else {
        let _ = autostart_manager.disable();
    }

    refresh_models(&state)?;
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    refresh_ui(&app)?;
    Ok(config)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub id: Option<String>,
    pub name: String,
    pub args: String,
}

#[tauri::command]
pub fn upsert_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ProfileInput,
) -> Result<AppConfig, String> {
    validate_profile_args(&input.args)?;
    if input.name.trim().is_empty() {
        return Err("Profile name cannot be empty".into());
    }

    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        if let Some(id) = input.id.as_ref().filter(|s| !s.is_empty()) {
            let profile = config
                .profiles
                .iter_mut()
                .find(|p| &p.id == id)
                .ok_or_else(|| "Profile not found".to_string())?;
            profile.name = input.name;
            profile.args = input.args;
        } else {
            let id = config::new_profile_id();
            config.profiles.push(Profile {
                id: id.clone(),
                name: input.name,
                args: input.args,
            });
            config.active_profile_id = id;
        }
        config::save_config(&state.app_data_dir, &config)?;
    }

    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    refresh_ui(&app)?;
    Ok(config)
}

#[tauri::command]
pub fn delete_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<AppConfig, String> {
    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        if config.profiles.len() <= 1 {
            return Err("Cannot delete the last profile".into());
        }
        config.profiles.retain(|p| p.id != profile_id);
        if config.active_profile_id == profile_id {
            config.active_profile_id = config.profiles[0].id.clone();
        }
        config::save_config(&state.app_data_dir, &config)?;
    }
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    refresh_ui(&app)?;
    Ok(config)
}

#[tauri::command]
pub fn set_active_profile(app: AppHandle, profile_id: String) -> Result<AppConfig, String> {
    set_active_profile_inner(&app, profile_id)?;
    let state = app.state::<AppState>();
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    Ok(config)
}

#[tauri::command]
pub fn set_active_model(
    app: AppHandle,
    model_path: Option<String>,
) -> Result<AppConfig, String> {
    set_active_model_inner(&app, model_path)?;
    let state = app.state::<AppState>();
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    Ok(config)
}

#[tauri::command]
pub fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelEntry>, String> {
    refresh_models(&state)
}

#[tauri::command]
pub fn get_status(server: State<'_, SharedServer>) -> Result<ServerStatus, String> {
    let status = server.lock().map_err(|e| e.to_string())?.status();
    Ok(status)
}

#[tauri::command]
pub fn start_server(app: AppHandle) -> Result<ServerStatus, String> {
    cmd_start_server(app.clone())?;
    let server = app.state::<SharedServer>();
    let status = server.lock().map_err(|e| e.to_string())?.status();
    Ok(status)
}

#[tauri::command]
pub fn stop_server(app: AppHandle) -> Result<ServerStatus, String> {
    cmd_stop_server(app.clone())?;
    let server = app.state::<SharedServer>();
    let status = server.lock().map_err(|e| e.to_string())?.status();
    Ok(status)
}

#[tauri::command]
pub fn load_model(app: AppHandle) -> Result<ServerStatus, String> {
    cmd_load_model(app.clone())?;
    let server = app.state::<SharedServer>();
    let status = server.lock().map_err(|e| e.to_string())?.status();
    Ok(status)
}

#[tauri::command]
pub fn unload_model(app: AppHandle) -> Result<ServerStatus, String> {
    cmd_unload_model(app.clone())?;
    let server = app.state::<SharedServer>();
    let status = server.lock().map_err(|e| e.to_string())?.status();
    Ok(status)
}

#[tauri::command]
pub fn show_settings(app: AppHandle) {
    tray::show_settings_window(&app);
}

#[tauri::command]
pub fn hide_settings(app: AppHandle) {
    tray::hide_settings_window(&app);
}

#[tauri::command]
pub fn get_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

fn cmd_start_server(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let server = app.state::<SharedServer>();
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    {
        let mut mgr = server.lock().map_err(|e| e.to_string())?;
        let with_model = config.active_model_path.is_some();
        mgr.start(&config, &state.app_data_dir, with_model)?;
    }
    refresh_ui(&app)
}

fn cmd_stop_server(app: AppHandle) -> Result<(), String> {
    let server = app.state::<SharedServer>();
    {
        let mut mgr = server.lock().map_err(|e| e.to_string())?;
        mgr.stop()?;
    }
    refresh_ui(&app)
}

fn cmd_load_model(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let server = app.state::<SharedServer>();
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    if config.active_model_path.is_none() {
        return Err("No model selected".into());
    }
    {
        let mut mgr = server.lock().map_err(|e| e.to_string())?;
        let phase = mgr.status().phase;
        if !matches!(
            phase,
            ServerPhase::RunningEmpty | ServerPhase::RunningLoaded | ServerPhase::Starting
        ) {
            return Err("Start the server first".into());
        }
        mgr.restart(&config, &state.app_data_dir, true)?;
    }
    refresh_ui(&app)
}

fn cmd_unload_model(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let server = app.state::<SharedServer>();
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    {
        let mut mgr = server.lock().map_err(|e| e.to_string())?;
        if mgr.status().phase != ServerPhase::RunningLoaded {
            return Err("Model is not loaded".into());
        }
        mgr.restart(&config, &state.app_data_dir, false)?;
    }
    refresh_ui(&app)
}

pub fn poll_server(app: &AppHandle) {
    let server = app.state::<SharedServer>();
    let mut status_snapshot = None;
    if let Ok(mut mgr) = server.lock() {
        let before = mgr.status().phase;
        let before_err = mgr.status().last_error.clone();
        mgr.poll();
        let after = mgr.status();
        if before != after.phase || before_err != after.last_error {
            status_snapshot = Some(after);
        }
    }
    if let Some(status) = status_snapshot {
        tray::emit_status(app, &status);
        let _ = refresh_ui(app);
    }
}

/// Used at startup to populate model cache.
pub fn initial_models(config: &AppConfig) -> Vec<ModelEntry> {
    if config.models_dir.trim().is_empty() {
        Vec::new()
    } else {
        models::scan_models(&config.models_dir).unwrap_or_default()
    }
}
