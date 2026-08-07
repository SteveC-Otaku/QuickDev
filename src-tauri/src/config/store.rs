use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub cwd: String,
    pub command: String,
    pub port: Option<u16>,
    pub open_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_highlight_ports")]
    pub highlight_ports: Vec<u16>,
    #[serde(default = "default_true")]
    pub auto_refresh: bool,
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default)]
    pub write_logs: bool,
    #[serde(default = "default_groups")]
    pub groups: Vec<String>,
    #[serde(default = "default_shell")]
    pub shell: String,
}

fn default_shell() -> String {
    "powershell".into()
}

fn default_groups() -> Vec<String> {
    vec!["工作".into(), "个人".into()]
}

fn default_hotkey() -> String {
    "Ctrl+Shift+D".into()
}

fn default_highlight_ports() -> Vec<u16> {
    vec![3000, 5173, 8080, 4173, 2283]
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            highlight_ports: default_highlight_ports(),
            auto_refresh: true,
            minimize_to_tray: true,
            auto_start: false,
            hotkey: default_hotkey(),
            write_logs: false,
            groups: default_groups(),
            shell: default_shell(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigBundle {
    version: u32,
    projects: Vec<Project>,
    settings: Settings,
}

pub fn export_all() -> Result<String, String> {
    let bundle = ConfigBundle {
        version: 1,
        projects: get_projects()?,
        settings: get_settings()?,
    };
    serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())
}

pub fn import_all(json: &str) -> Result<(), String> {
    let bundle: ConfigBundle = serde_json::from_str(json).map_err(|e| e.to_string())?;
    save_settings(bundle.settings)?;
    let projects_path = data_dir()?.join("projects.json");
    let projects_json =
        serde_json::to_string_pretty(&bundle.projects).map_err(|e| e.to_string())?;
    atomic_write(&projects_path, &projects_json)
}

fn data_dir() -> Result<PathBuf, String> {
    let base = std::env::var("LOCALAPPDATA").map_err(|e| e.to_string())?;
    let dir = PathBuf::from(base).join("QuickDev");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

// ponytail: tmp+rename atomic write; upgrade to file lock if concurrent writes matter
fn atomic_write(path: &PathBuf, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &PathBuf, default: T) -> Result<T, String> {
    if !path.exists() {
        return Ok(default);
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn get_projects() -> Result<Vec<Project>, String> {
    let path = data_dir()?.join("projects.json");
    read_json(&path, Vec::new())
}

pub fn save_project(project: Project) -> Result<(), String> {
    let path = data_dir()?.join("projects.json");
    let mut projects = get_projects()?;
    if let Some(idx) = projects.iter().position(|p| p.id == project.id) {
        projects[idx] = project;
    } else {
        projects.push(project);
    }
    let json = serde_json::to_string_pretty(&projects).map_err(|e| e.to_string())?;
    atomic_write(&path, &json)
}

pub fn delete_project(id: &str) -> Result<(), String> {
    let path = data_dir()?.join("projects.json");
    let mut projects = get_projects()?;
    projects.retain(|p| p.id != id);
    let json = serde_json::to_string_pretty(&projects).map_err(|e| e.to_string())?;
    atomic_write(&path, &json)
}

pub fn get_settings() -> Result<Settings, String> {
    let path = data_dir()?.join("settings.json");
    read_json(&path, Settings::default())
}

pub fn save_settings(settings: Settings) -> Result<(), String> {
    let path = data_dir()?.join("settings.json");
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    atomic_write(&path, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_ports() {
        let s = Settings::default();
        assert!(s.highlight_ports.contains(&3000));
        assert!(s.highlight_ports.contains(&5173));
    }
}
