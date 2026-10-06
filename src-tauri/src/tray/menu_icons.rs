// Generated from bundled Lucide A SVG assets, see icons/menu/LICENSE.txt.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::image::Image;
pub(super) fn icon(id: &str, text: &str, size: u32) -> Option<Image<'static>> {
    let action = if id.starts_with("open|") && text.contains("App") { "show" } else if id.starts_with("plan_start|") { "start" } else if id.starts_with("plan_stop|") { "stop" } else { id.split('|').next()? };
    static CACHE: OnceLock<Mutex<HashMap<(String,u32), Image<'static>>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
    let key = (action.to_owned(),size);
    if let Some(icon) = cache.get(&key) { return Some(icon.clone()); }
    let bytes: &[u8] = match (action,size) {
        ("show", 16) => include_bytes!("../../icons/menu/app-16.png"),
        ("show", 20) => include_bytes!("../../icons/menu/app-20.png"),
        ("show", 24) => include_bytes!("../../icons/menu/app-24.png"),
        ("show", 32) => include_bytes!("../../icons/menu/app-32.png"),
        ("show", 40) => include_bytes!("../../icons/menu/app-40.png"),
        ("show", 48) => include_bytes!("../../icons/menu/app-48.png"),
        ("add-project", 16) => include_bytes!("../../icons/menu/plus-16.png"),
        ("add-project", 20) => include_bytes!("../../icons/menu/plus-20.png"),
        ("add-project", 24) => include_bytes!("../../icons/menu/plus-24.png"),
        ("add-project", 32) => include_bytes!("../../icons/menu/plus-32.png"),
        ("add-project", 40) => include_bytes!("../../icons/menu/plus-40.png"),
        ("add-project", 48) => include_bytes!("../../icons/menu/plus-48.png"),
        ("refresh-ports", 16) => include_bytes!("../../icons/menu/refresh-16.png"),
        ("refresh-ports", 20) => include_bytes!("../../icons/menu/refresh-20.png"),
        ("refresh-ports", 24) => include_bytes!("../../icons/menu/refresh-24.png"),
        ("refresh-ports", 32) => include_bytes!("../../icons/menu/refresh-32.png"),
        ("refresh-ports", 40) => include_bytes!("../../icons/menu/refresh-40.png"),
        ("refresh-ports", 48) => include_bytes!("../../icons/menu/refresh-48.png"),
        ("settings", 16) => include_bytes!("../../icons/menu/settings-16.png"),
        ("settings", 20) => include_bytes!("../../icons/menu/settings-20.png"),
        ("settings", 24) => include_bytes!("../../icons/menu/settings-24.png"),
        ("settings", 32) => include_bytes!("../../icons/menu/settings-32.png"),
        ("settings", 40) => include_bytes!("../../icons/menu/settings-40.png"),
        ("settings", 48) => include_bytes!("../../icons/menu/settings-48.png"),
        ("start", 16) => include_bytes!("../../icons/menu/play-16.png"),
        ("start", 20) => include_bytes!("../../icons/menu/play-20.png"),
        ("start", 24) => include_bytes!("../../icons/menu/play-24.png"),
        ("start", 32) => include_bytes!("../../icons/menu/play-32.png"),
        ("start", 40) => include_bytes!("../../icons/menu/play-40.png"),
        ("start", 48) => include_bytes!("../../icons/menu/play-48.png"),
        ("stop", 16) => include_bytes!("../../icons/menu/stop-16.png"),
        ("stop", 20) => include_bytes!("../../icons/menu/stop-20.png"),
        ("stop", 24) => include_bytes!("../../icons/menu/stop-24.png"),
        ("stop", 32) => include_bytes!("../../icons/menu/stop-32.png"),
        ("stop", 40) => include_bytes!("../../icons/menu/stop-40.png"),
        ("stop", 48) => include_bytes!("../../icons/menu/stop-48.png"),
        ("open", 16) => include_bytes!("../../icons/menu/open-16.png"),
        ("open", 20) => include_bytes!("../../icons/menu/open-20.png"),
        ("open", 24) => include_bytes!("../../icons/menu/open-24.png"),
        ("open", 32) => include_bytes!("../../icons/menu/open-32.png"),
        ("open", 40) => include_bytes!("../../icons/menu/open-40.png"),
        ("open", 48) => include_bytes!("../../icons/menu/open-48.png"),
        ("logs", 16) => include_bytes!("../../icons/menu/log-16.png"),
        ("logs", 20) => include_bytes!("../../icons/menu/log-20.png"),
        ("logs", 24) => include_bytes!("../../icons/menu/log-24.png"),
        ("logs", 32) => include_bytes!("../../icons/menu/log-32.png"),
        ("logs", 40) => include_bytes!("../../icons/menu/log-40.png"),
        ("logs", 48) => include_bytes!("../../icons/menu/log-48.png"),
        ("quit-retain", 16) => include_bytes!("../../icons/menu/power-16.png"),
        ("quit-retain", 20) => include_bytes!("../../icons/menu/power-20.png"),
        ("quit-retain", 24) => include_bytes!("../../icons/menu/power-24.png"),
        ("quit-retain", 32) => include_bytes!("../../icons/menu/power-32.png"),
        ("quit-retain", 40) => include_bytes!("../../icons/menu/power-40.png"),
        ("quit-retain", 48) => include_bytes!("../../icons/menu/power-48.png"),
        ("quit-stop", 16) => include_bytes!("../../icons/menu/stop-16.png"),
        ("quit-stop", 20) => include_bytes!("../../icons/menu/stop-20.png"),
        ("quit-stop", 24) => include_bytes!("../../icons/menu/stop-24.png"),
        ("quit-stop", 32) => include_bytes!("../../icons/menu/stop-32.png"),
        ("quit-stop", 40) => include_bytes!("../../icons/menu/stop-40.png"),
        ("quit-stop", 48) => include_bytes!("../../icons/menu/stop-48.png"),
        ("stop-all", 16) => include_bytes!("../../icons/menu/stop-16.png"),
        ("stop-all", 20) => include_bytes!("../../icons/menu/stop-20.png"),
        ("stop-all", 24) => include_bytes!("../../icons/menu/stop-24.png"),
        ("stop-all", 32) => include_bytes!("../../icons/menu/stop-32.png"),
        ("stop-all", 40) => include_bytes!("../../icons/menu/stop-40.png"),
        ("stop-all", 48) => include_bytes!("../../icons/menu/stop-48.png"),
        _ => return None,
    };
    let rgba = image::load_from_memory(bytes).ok()?.into_rgba8();
    let icon = Image::new_owned(rgba.into_raw(),size,size);
    cache.insert(key,icon.clone());
    Some(icon)
}
