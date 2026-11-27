use std::sync::atomic::AtomicBool;
use tauri::Manager;

pub mod profiles;
pub mod tray;
pub mod windows;

pub struct AppState {
    pub quitting: AtomicBool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub auto_open: bool,
    pub is_open: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuntimeStats {
    pub ram_mb: f64,
    pub open_windows: usize,
    pub platform: &'static str,
    pub version: String,
}

impl ProfileView {
    fn from_profile(app: &tauri::AppHandle, profile: profiles::Profile) -> Self {
        let is_open = windows::is_profile_window_open(app, &profile.id);
        Self {
            id: profile.id,
            name: profile.name,
            auto_open: profile.auto_open,
            is_open,
        }
    }
}

#[tauri::command]
fn cmd_list_profiles(app: tauri::AppHandle) -> Result<Vec<ProfileView>, String> {
    let list = profiles::list_profiles(&app)?;
    Ok(list
        .into_iter()
        .map(|p| ProfileView::from_profile(&app, p))
        .collect())
}

#[tauri::command]
fn cmd_create_profile(
    app: tauri::AppHandle,
    name: String,
    auto_open: bool,
) -> Result<ProfileView, String> {
    let created = profiles::create_profile(&app, &name, auto_open)?;
    Ok(ProfileView::from_profile(&app, created))
}

#[tauri::command]
fn cmd_open_profile(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let profile = profiles::get_profile(&app, &id)?;
    windows::open_or_focus_profile_window(&app, &profile)
}

#[tauri::command]
fn cmd_toggle_auto_open(
    app: tauri::AppHandle,
    id: String,
    auto_open: bool,
) -> Result<ProfileView, String> {
    let updated = profiles::toggle_auto_open(&app, &id, auto_open)?;
    Ok(ProfileView::from_profile(&app, updated))
}

#[tauri::command]
fn cmd_trim_ram() -> Result<f64, String> {
    windows::trim_current_process_ram()
}

#[tauri::command]
fn cmd_runtime_stats(app: tauri::AppHandle) -> RuntimeStats {
    let open_windows = app
        .webview_windows()
        .keys()
        .filter(|label| label.starts_with("zalo-"))
        .count();
    RuntimeStats {
        ram_mb: windows::get_process_memory_mb(),
        open_windows,
        platform: std::env::consts::OS,
        version: app.package_info().version.to_string(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            quitting: AtomicBool::new(false),
        })
        .setup(|app| {
            tray::setup_tray(app.handle())?;

            if let Some(main_win) = app.get_webview_window("main") {
                windows::attach_hide_on_close(&main_win, app.handle());
            }

            if let Ok(list) = profiles::list_profiles(app.handle()) {
                for profile in list.into_iter().filter(|p| p.auto_open) {
                    let _ = windows::open_or_focus_profile_window(app.handle(), &profile);
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_list_profiles,
            cmd_create_profile,
            cmd_open_profile,
            cmd_toggle_auto_open,
            cmd_trim_ram,
            cmd_runtime_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
