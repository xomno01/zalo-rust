use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

pub mod profiles;
pub mod tray;
pub mod windows;

pub struct AppState {
    pub quitting: AtomicBool,
}

pub(crate) fn log_event(app: &tauri::AppHandle, msg: &str) {
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let log_path = dir.join("runtime.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = writeln!(file, "{}", msg);
    }
}

pub(crate) fn attach_main_close_handler(window: &tauri::WebviewWindow, app: &tauri::AppHandle) {
    let win_for_event = window.clone();
    let app_for_event = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            let state = app_for_event.state::<AppState>();
            let quitting = state.quitting.load(Ordering::SeqCst);
            if quitting {
                return;
            }
            let has_chat_windows = app_for_event
                .webview_windows()
                .keys()
                .any(|label| label.starts_with("zalo-"));
            if has_chat_windows {
                api.prevent_close();
                let _ = win_for_event.hide();
                let _ = windows::trim_current_process_ram();
            } else {
                state.quitting.store(true, Ordering::SeqCst);
                log_event(&app_for_event, "[EXIT] Main window closed with 0 active chat windows");
            }
        }
    });
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

fn extract_open_profile_arg<I, S>(args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().find_map(|arg| {
        arg.as_ref()
            .strip_prefix("--open-profile=")
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
    })
}

#[tauri::command]
async fn cmd_open_profile(app: tauri::AppHandle, id: String) -> Result<(), String> {
    log_event(&app, &format!("[CMD] open_profile id={}", id));
    let profile = profiles::get_profile(&app, &id)?;
    let res = windows::open_or_focus_profile_window(&app, &profile);
    if let Err(ref e) = res {
        log_event(&app, &format!("[ERROR] open_profile id={} err={}", id, e));
    }
    res
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

fn open_profile_by_id(app: &tauri::AppHandle, id: &str) {
    if let Ok(profile) = profiles::get_profile(app, id) {
        let _ = windows::open_or_focus_profile_window(app, &profile);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            log_event(app, &format!("[SINGLE-INSTANCE] wakeup args={:?}", args));
            tray::show_and_focus_main(app);
            if let Some(profile_id) = extract_open_profile_arg(&args) {
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    open_profile_by_id(&app_clone, &profile_id);
                });
            }
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            quitting: AtomicBool::new(false),
        })
        .setup(|app| {
            log_event(app.handle(), "[STARTUP] Zalo Rust v1.1.0 initialized");
            tray::setup_tray(app.handle())?;

            if let Some(main_win) = app.get_webview_window("main") {
                attach_main_close_handler(&main_win, app.handle());
            }
            tray::show_and_focus_main(app.handle());

            if let Ok(list) = profiles::list_profiles(app.handle()) {
                for profile in list.into_iter().filter(|p| p.auto_open) {
                    let _ = windows::open_or_focus_profile_window(app.handle(), &profile);
                }
            }

            if let Some(profile_id) = extract_open_profile_arg(std::env::args()) {
                open_profile_by_id(app.handle(), &profile_id);
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

#[cfg(test)]
mod tests {
    use super::extract_open_profile_arg;

    #[test]
    fn extracts_open_profile_arg_cleanly() {
        assert_eq!(
            extract_open_profile_arg(["zalo-rust.exe", "--open-profile=work"]),
            Some("work".to_string())
        );
        assert_eq!(extract_open_profile_arg(["zalo-rust.exe"]), None);
        assert_eq!(extract_open_profile_arg(["--open-profile="]), None);
    }
}
