use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, WebviewWindow};

use crate::error::{AppError, AppResult};

/// When true, CloseRequested may destroy the window / exit the process.
static ALLOW_EXIT: AtomicBool = AtomicBool::new(false);
static EXIT_PREPARED: AtomicBool = AtomicBool::new(false);

/// Intercept user close (show confirm / keep running) unless quitting or UI recreate.
pub fn should_intercept_close() -> bool {
    !ALLOW_EXIT.load(Ordering::SeqCst) && !crate::commands::ui_memory::should_prevent_exit()
}

fn main_window(app: &AppHandle) -> AppResult<WebviewWindow> {
    app.get_webview_window("main")
        .or_else(|| {
            app.webview_windows()
                .into_iter()
                .find(|(label, _)| !label.starts_with("__"))
                .map(|(_, w)| w)
        })
        .ok_or_else(|| AppError::Message("找不到主窗口".into()))
}

#[tauri::command]
pub fn hide_to_tray(app: AppHandle) -> AppResult<()> {
    let window = main_window(&app)?;
    window
        .hide()
        .map_err(|err| AppError::Message(format!("隐藏窗口失败: {err}")))?;
    Ok(())
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) -> AppResult<()> {
    let window = main_window(&app)?;
    let _ = window.unminimize();
    window
        .show()
        .map_err(|err| AppError::Message(format!("显示窗口失败: {err}")))?;
    let _ = window.set_focus();
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) -> AppResult<()> {
    arm_allow_exit();
    if let Err(error) = prepare_for_exit(&app) {
        eprintln!("failed to prepare application exit: {error}");
    }
    app.exit(0);
    Ok(())
}

pub fn arm_allow_exit() {
    ALLOW_EXIT.store(true, Ordering::SeqCst);
}

pub fn is_exit_prepared() -> bool {
    EXIT_PREPARED.load(Ordering::SeqCst)
}

/// Capture the restart snapshot and stop all in-process listeners exactly once.
/// Hiding the window does not call this function, so background mode is kept.
pub fn prepare_for_exit(app: &AppHandle) -> AppResult<()> {
    if EXIT_PREPARED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let capture_result = crate::startup_restore::capture_running_services(app);
    let shutdown_result = app
        .state::<crate::app_state::AppState>()
        .with_runtime(|runtime| {
            runtime.shutdown_all();
            Ok(())
        });

    capture_result.and(shutdown_result)
}
