use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_opener::OpenerExt;

const ALLOWED_ZALO_DOMAINS: &[&str] = &["zalo.me", "zaloapp.com", "zing.vn", "zadn.vn", "zdn.vn"];

/// Checks whether a URL is permitted inside an isolated Zalo WebView window.
pub fn is_allowed_zalo_url(url: &url::Url) -> bool {
    if url.as_str() == "about:blank" {
        return true;
    }
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host_lower = host.to_ascii_lowercase();
    ALLOWED_ZALO_DOMAINS
        .iter()
        .any(|domain| host_lower == *domain || host_lower.ends_with(&format!(".{}", domain)))
}

/// Returns true if the WebView window for the given profile ID is currently open.
pub fn is_profile_window_open(app: &tauri::AppHandle, id: &str) -> bool {
    app.get_webview_window(&format!("zalo-{}", id)).is_some()
}

/// Attaches a CloseRequested handler that hides the window to tray and trims RAM unless quitting.
pub fn attach_hide_on_close(window: &WebviewWindow, app: &tauri::AppHandle) {
    let win_for_event = window.clone();
    let app_for_event = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            let state = app_for_event.state::<crate::AppState>();
            let quitting = state.quitting.load(std::sync::atomic::Ordering::SeqCst);
            if !quitting {
                api.prevent_close();
                let _ = win_for_event.hide();
                let _ = trim_current_process_ram();
            }
        }
    });
}

/// Opens an isolated Zalo WebView window for the profile, or focuses it if already open.
pub fn open_or_focus_profile_window(
    app: &tauri::AppHandle,
    profile: &crate::profiles::Profile,
) -> Result<(), String> {
    let label = format!("zalo-{}", profile.id);
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.show();
        let _ = existing.unminimize();
        existing
            .set_focus()
            .map_err(|e| format!("Không thể focus cửa sổ Zalo: {}", e))?;
        return Ok(());
    }

    let zalo_url: url::Url = "https://chat.zalo.me/"
        .parse()
        .map_err(|e| format!("URL Zalo không hợp lệ: {}", e))?;

    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(zalo_url))
        .title(format!("Zalo Ultra — {}", profile.name))
        .inner_size(1180.0, 780.0)
        .min_inner_size(800.0, 600.0);

    #[cfg(not(target_os = "macos"))]
    {
        let data_dir = crate::profiles::profile_data_dir(app, &profile.id)?;
        builder = builder.data_directory(data_dir);
    }

    #[cfg(target_os = "macos")]
    {
        builder = builder.data_store_identifier(crate::profiles::profile_store_uuid(&profile.id));
    }

    let app_for_nav = app.clone();
    builder = builder.on_navigation(move |nav_url| {
        if is_allowed_zalo_url(nav_url) {
            return true;
        }
        if nav_url.scheme() == "http" || nav_url.scheme() == "https" {
            let _ = app_for_nav
                .opener()
                .open_url(nav_url.as_str(), None::<&str>);
        }
        false
    });

    let window = builder
        .build()
        .map_err(|e| format!("Không thể tạo cửa sổ Zalo: {}", e))?;
    attach_hide_on_close(&window, app);
    Ok(())
}

/// Returns the current process working set memory in megabytes.
#[cfg(windows)]
pub fn get_process_memory_mb() -> f64 {
    use std::mem;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    unsafe {
        let handle = GetCurrentProcess();
        let mut counters: PROCESS_MEMORY_COUNTERS = mem::zeroed();
        counters.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(handle, &mut counters, counters.cb) != 0 {
            counters.WorkingSetSize as f64 / (1024.0 * 1024.0)
        } else {
            0.0
        }
    }
}

/// Trims the current process working set on Windows and returns updated memory in MB.
#[cfg(windows)]
pub fn trim_current_process_ram() -> Result<f64, String> {
    use windows_sys::Win32::System::ProcessStatus::EmptyWorkingSet;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let ok = unsafe { EmptyWorkingSet(GetCurrentProcess()) };
    if ok == 0 {
        return Err("Không thể nén RAM tiến trình (EmptyWorkingSet failed)".to_string());
    }
    Ok(get_process_memory_mb())
}

#[cfg(not(windows))]
pub fn get_process_memory_mb() -> f64 {
    0.0
}

#[cfg(not(windows))]
pub fn trim_current_process_ram() -> Result<f64, String> {
    Ok(0.0)
}

#[cfg(test)]
mod tests {
    use super::is_allowed_zalo_url;

    #[test]
    fn allows_valid_zalo_urls_and_about_blank() {
        assert!(is_allowed_zalo_url(&"about:blank".parse().unwrap()));
        assert!(is_allowed_zalo_url(&"https://chat.zalo.me/".parse().unwrap()));
        assert!(is_allowed_zalo_url(&"https://id.zalo.me/account".parse().unwrap()));
        assert!(is_allowed_zalo_url(&"https://zalo.me".parse().unwrap()));
        assert!(is_allowed_zalo_url(&"https://stc-zaloprofile.zdn.vn/a.png".parse().unwrap()));
        assert!(is_allowed_zalo_url(&"https://files.zadn.vn/file".parse().unwrap()));
        assert!(is_allowed_zalo_url(&"https://login.zaloapp.com/".parse().unwrap()));
        assert!(is_allowed_zalo_url(&"https://api.zing.vn/".parse().unwrap()));
    }

    #[test]
    fn blocks_external_and_spoofed_domains() {
        assert!(!is_allowed_zalo_url(&"https://google.com/".parse().unwrap()));
        assert!(!is_allowed_zalo_url(&"https://evilzalo.me/".parse().unwrap()));
        assert!(!is_allowed_zalo_url(&"http://chat.zalo.me/".parse().unwrap()));
        assert!(!is_allowed_zalo_url(&"file:///etc/passwd".parse().unwrap()));
    }
}
