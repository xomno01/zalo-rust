use std::sync::atomic::Ordering;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub(crate) fn show_and_focus_main(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = app_handle.get_webview_window("main") {
            let _ = win.unminimize();
            let _ = win.show();
            let _ = win.set_always_on_top(true);
            let _ = win.set_focus();
            let _ = win.set_always_on_top(false);
            return;
        }

        if let Ok(win) = tauri::WebviewWindowBuilder::new(
            &app_handle,
            "main",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("Zalo Rust")
        .inner_size(540.0, 640.0)
        .min_inner_size(460.0, 520.0)
        .center()
        .build()
        {
            crate::attach_main_close_handler(&win, &app_handle);
            let _ = win.show();
            let _ = win.set_always_on_top(true);
            let _ = win.set_focus();
            let _ = win.set_always_on_top(false);
        }
    });
}

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            show_and_focus_main(app);
        }
    }
}

/// Initializes the system tray icon, menu actions, and left-click toggle behavior.
pub fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show_manager = MenuItem::with_id(app, "show_manager", "Quản lý tài khoản", true, None::<&str>)?;
    let open_default = MenuItem::with_id(app, "open_default", "Mở tài khoản mặc định", true, None::<&str>)?;
    let trim_ram = MenuItem::with_id(app, "trim_ram", "Nén RAM ngay", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Thoát hoàn toàn", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_manager, &open_default, &trim_ram, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("missing default window icon for tray");

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("Zalo Rust")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show_manager" => {
                show_and_focus_main(app);
            }
            "open_default" => {
                let target = crate::profiles::get_profile(app, "default").or_else(|_| {
                    crate::profiles::list_profiles(app)?
                        .into_iter()
                        .next()
                        .ok_or_else(|| "Không có tài khoản nào".to_string())
                });
                if let Ok(profile) = target {
                    let _ = crate::windows::open_or_focus_profile_window(app, &profile);
                }
            }
            "trim_ram" => {
                let _ = crate::windows::trim_current_process_ram();
            }
            "quit" => {
                let state = app.state::<crate::AppState>();
                state.quitting.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}
