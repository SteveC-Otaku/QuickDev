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

    let shortcut = parse_hotkey(&hotkey)?;

    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| format!("取消热键注册失败: {e}"))?;
    let Some(shortcut) = shortcut else { return Ok(()); };
    gs.on_shortcut(shortcut, |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            tray::show_main_window(_app);
        }
    })
    .map_err(|e| format!("热键注册失败（可能已被占用）: {e}"))?;
    Ok(())
}

fn parse_hotkey(hotkey: &str) -> Result<Option<Shortcut>, String> {
    let hotkey = hotkey.trim();
    if hotkey.is_empty() { return Ok(None); }
    hotkey.parse().map(Some).map_err(|_| format!("无效热键格式: {hotkey}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_hotkey_keeps_blank_disabled_and_rejects_invalid_input() {
        assert!(parse_hotkey("").unwrap().is_none());
        assert!(parse_hotkey("   ").unwrap().is_none());
        assert!(parse_hotkey(" Ctrl+Shift+D ").unwrap().is_some());
        assert!(parse_hotkey("invalid-key-name").is_err());
    }
}
