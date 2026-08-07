use crate::config::store::get_settings;
use crate::tray;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const DEFAULT_HOTKEY: &str = "Ctrl+Shift+D";

pub fn setup(app: &AppHandle) -> Result<(), String> {
    apply(app)
}

pub fn apply(app: &AppHandle) -> Result<(), String> {
    let hotkey = get_settings()
        .map(|s| s.hotkey)
        .unwrap_or_else(|_| DEFAULT_HOTKEY.into());

    let shortcut: Shortcut = hotkey
        .parse()
        .map_err(|_| format!("无效热键格式: {hotkey}"))?;

    let gs = app.global_shortcut();
    gs.unregister_all().ok();
    gs.on_shortcut(shortcut, |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            tray::show_main_window(_app);
        }
    })
    .map_err(|e| format!("热键注册失败（可能已被占用）: {e}"))?;
    Ok(())
}
