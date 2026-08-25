use crate::config::AppConfig;
use crate::models::ModelEntry;
use crate::server::{ServerPhase, ServerStatus};
use std::collections::BTreeMap;
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

pub const TRAY_ID: &str = "main-tray";

pub struct TrayState {
    pub tray: Option<TrayIcon>,
}

impl Default for TrayState {
    fn default() -> Self {
        Self { tray: None }
    }
}

fn phase_label(phase: ServerPhase) -> &'static str {
    match phase {
        ServerPhase::Stopped => "Server: stopped",
        ServerPhase::Starting => "Server: starting…",
        ServerPhase::RunningEmpty => "Server: running (no model)",
        ServerPhase::RunningLoaded => "Server: running",
        ServerPhase::Error => "Server: error",
    }
}

fn tooltip(status: &ServerStatus) -> String {
    let base = phase_label(status.phase);
    match (&status.loaded_model, &status.url) {
        (Some(m), Some(u)) => {
            let name = std::path::Path::new(m)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(m);
            format!("{base}\n{name}\n{u}")
        }
        (None, Some(u)) => format!("{base}\n{u}"),
        _ => base.to_string(),
    }
}

/// 16x16 solid icon with a status-colored circle.
fn status_icon(phase: ServerPhase) -> Image<'static> {
    const SIZE: u32 = 16;
    let (r, g, b) = match phase {
        ServerPhase::Stopped => (120u8, 120u8, 120u8),
        ServerPhase::Starting => (220, 180, 40),
        ServerPhase::RunningEmpty | ServerPhase::RunningLoaded => (40, 180, 80),
        ServerPhase::Error => (200, 50, 50),
    };

    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    let cx = (SIZE as f32 - 1.0) / 2.0;
    let cy = cx;
    let radius = 6.0;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * SIZE + x) * 4) as usize;
            if dist <= radius {
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            } else if dist <= radius + 1.0 {
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 120;
            }
        }
    }

    Image::new_owned(rgba, SIZE, SIZE)
}

pub fn show_settings_window(app: &AppHandle) {
    let _ = crate::commands::refresh_models_for_menu(app);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_skip_taskbar(false);
        let _ = win.show();
        let _ = win.set_focus();
    }
}

pub fn hide_settings_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
        let _ = win.set_skip_taskbar(true);
    }
}

pub fn build_menu(
    app: &AppHandle,
    config: &AppConfig,
    models: &[ModelEntry],
    status: &ServerStatus,
) -> tauri::Result<Menu<tauri::Wry>> {
    let status_item = MenuItem::with_id(
        app,
        "status",
        phase_label(status.phase),
        false,
        None::<&str>,
    )?;

    let can_start = matches!(status.phase, ServerPhase::Stopped | ServerPhase::Error);
    let can_stop = matches!(
        status.phase,
        ServerPhase::Starting | ServerPhase::RunningEmpty | ServerPhase::RunningLoaded
    );
    let can_load =
        can_stop && config.active_model_path.is_some() && status.phase != ServerPhase::Starting;
    let can_unload = matches!(status.phase, ServerPhase::RunningLoaded);

    let start = MenuItem::with_id(app, "start", "Start", can_start, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "Stop", can_stop, None::<&str>)?;
    let load = MenuItem::with_id(app, "load", "Load model", can_load, None::<&str>)?;
    let unload = MenuItem::with_id(app, "unload", "Unload model", can_unload, None::<&str>)?;

    let mut groups: BTreeMap<String, Vec<&ModelEntry>> = BTreeMap::new();
    for m in models {
        groups.entry(m.group.clone()).or_default().push(m);
    }

    let model_submenu = if models.is_empty() {
        let empty = MenuItem::with_id(
            app,
            "models_empty",
            "(no models — set models folder)",
            false,
            None::<&str>,
        )?;
        Submenu::with_items(app, "Model", true, &[&empty])?
    } else if groups.len() == 1 && groups.keys().next().map(|k| k.as_str()) == Some("(root)") {
        let items: Vec<CheckMenuItem<tauri::Wry>> = models
            .iter()
            .map(|m| {
                let checked = config
                    .active_model_path
                    .as_ref()
                    .map(|p| p == &m.path)
                    .unwrap_or(false);
                CheckMenuItem::with_id(
                    app,
                    format!("model:{}", m.path),
                    &m.name,
                    true,
                    checked,
                    None::<&str>,
                )
                .expect("model check item")
            })
            .collect();
        let refs: Vec<&dyn IsMenuItem<tauri::Wry>> =
            items.iter().map(|i| i as &dyn IsMenuItem<tauri::Wry>).collect();
        Submenu::with_items(app, "Model", true, &refs)?
    } else {
        let mut submenus: Vec<Submenu<tauri::Wry>> = Vec::new();
        for (group, entries) in &groups {
            let items: Vec<CheckMenuItem<tauri::Wry>> = entries
                .iter()
                .map(|m| {
                    let checked = config
                        .active_model_path
                        .as_ref()
                        .map(|p| p == &m.path)
                        .unwrap_or(false);
                    let label = if group.as_str() == "(root)" {
                        m.name.clone()
                    } else {
                        m.relative_path
                            .strip_prefix(&format!("{group}/"))
                            .unwrap_or(&m.relative_path)
                            .to_string()
                    };
                    CheckMenuItem::with_id(
                        app,
                        format!("model:{}", m.path),
                        label,
                        true,
                        checked,
                        None::<&str>,
                    )
                    .expect("model check item")
                })
                .collect();
            let refs: Vec<&dyn IsMenuItem<tauri::Wry>> =
                items.iter().map(|i| i as &dyn IsMenuItem<tauri::Wry>).collect();
            submenus.push(Submenu::with_items(app, group, true, &refs)?);
        }
        let refs: Vec<&dyn IsMenuItem<tauri::Wry>> = submenus
            .iter()
            .map(|s| s as &dyn IsMenuItem<tauri::Wry>)
            .collect();
        Submenu::with_items(app, "Model", true, &refs)?
    };

    let profile_items: Vec<CheckMenuItem<tauri::Wry>> = config
        .profiles
        .iter()
        .map(|p| {
            let checked = p.id == config.active_profile_id;
            CheckMenuItem::with_id(
                app,
                format!("profile:{}", p.id),
                &p.name,
                true,
                checked,
                None::<&str>,
            )
            .expect("profile check item")
        })
        .collect();
    let profile_refs: Vec<&dyn IsMenuItem<tauri::Wry>> = profile_items
        .iter()
        .map(|i| i as &dyn IsMenuItem<tauri::Wry>)
        .collect();
    let profile_submenu = if profile_refs.is_empty() {
        let empty =
            MenuItem::with_id(app, "profiles_empty", "(no profiles)", false, None::<&str>)?;
        Submenu::with_items(app, "Profile", true, &[&empty])?
    } else {
        Submenu::with_items(app, "Profile", true, &profile_refs)?
    };

    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &status_item,
            &sep1,
            &start,
            &stop,
            &load,
            &unload,
            &sep2,
            &model_submenu,
            &profile_submenu,
            &sep3,
            &settings,
            &quit,
        ],
    )
}

pub fn rebuild_tray(
    app: &AppHandle,
    config: &AppConfig,
    models: &[ModelEntry],
    status: &ServerStatus,
) -> tauri::Result<()> {
    let menu = build_menu(app, config, models, status)?;
    let icon = status_icon(status.phase);
    let tip = tooltip(status);

    let state = app.state::<Mutex<TrayState>>();
    let mut guard = state.lock().expect("tray state");

    if let Some(tray) = guard.tray.as_ref() {
        tray.set_menu(Some(menu))?;
        tray.set_icon(Some(icon))?;
        tray.set_tooltip(Some(tip))?;
    } else {
        let tray = TrayIconBuilder::with_id(TRAY_ID)
            .icon(icon)
            .menu(&menu)
            .tooltip(tip)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| {
                crate::commands::handle_tray_menu(app, event.id.as_ref());
            })
            .on_tray_icon_event(|tray, event| {
                match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Right,
                        button_state: MouseButtonState::Down,
                        ..
                    } => {
                        let _ = crate::commands::refresh_models_for_menu(tray.app_handle());
                    }
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } => {
                        show_settings_window(tray.app_handle());
                    }
                    _ => {}
                }
            })
            .build(app)?;
        guard.tray = Some(tray);
    }
    Ok(())
}

pub fn emit_status(app: &AppHandle, status: &ServerStatus) {
    let _ = app.emit("app://status", status);
}

pub fn emit_models(app: &AppHandle, models: &[ModelEntry]) {
    let _ = app.emit("app://models", models);
}
