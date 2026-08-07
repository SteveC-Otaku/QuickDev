use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectResult {
    pub name: Option<String>,
    pub command: String,
    pub package_manager: String,
    pub port: Option<u16>,
    pub dev_script: Option<String>,
}

pub fn resolve_project_root(cwd: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(cwd);
    if !path.is_dir() {
        return Err("路径不存在或不是目录".into());
    }
    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("src-tauri"))
    {
        if let Some(parent) = path.parent() {
            if parent.join("package.json").exists() {
                return Ok(parent.to_path_buf());
            }
        }
    }
    Ok(path)
}

pub fn detect_project(cwd: &str) -> Result<DetectResult, String> {
    let root = resolve_project_root(cwd)?;
    let pkg_path = root.join("package.json");
    let raw = fs::read_to_string(&pkg_path).map_err(|_| "未找到 package.json")?;
    let pkg: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    let dev_script = pkg["scripts"]["dev"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "package.json 中未找到 scripts.dev".to_string())?;

    let pm = detect_package_manager(&root);
    let is_tauri = root.join("src-tauri").is_dir() && pkg["scripts"]["tauri"].is_string();
    let command = if is_tauri {
        tauri_dev_command(pm)
    } else {
        dev_command(pm)
    };
    let name = pkg["name"].as_str().map(humanize_name);
    let port = detect_tauri_port(&root).or_else(|| guess_port(&dev_script));

    Ok(DetectResult {
        name,
        command,
        package_manager: pm.to_string(),
        port,
        dev_script: Some(dev_script),
    })
}

fn detect_package_manager(root: &Path) -> &'static str {
    if root.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if root.join("yarn.lock").exists() {
        "yarn"
    } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
        "bun"
    } else {
        "npm"
    }
}

fn dev_command(pm: &str) -> String {
    match pm {
        "npm" | "bun" => format!("{pm} run dev"),
        _ => format!("{pm} dev"),
    }
}

fn tauri_dev_command(pm: &str) -> String {
    match pm {
        "npm" | "bun" => format!("{pm} run tauri dev"),
        _ => format!("{pm} tauri dev"),
    }
}

fn detect_tauri_port(root: &Path) -> Option<u16> {
    let conf_path = root.join("src-tauri/tauri.conf.json");
    let raw = fs::read_to_string(conf_path).ok()?;
    let conf: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let dev_url = conf["build"]["devUrl"].as_str()?;
    port_from_url(dev_url)
}

fn port_from_url(url: &str) -> Option<u16> {
    let after_scheme = url.split("://").nth(1)?;
    let host_port = after_scheme.split('/').next()?;
    host_port.rsplit(':').next()?.parse().ok()
}

fn humanize_name(raw: &str) -> String {
    raw.replace(['-', '_'], " ")
}

fn guess_port(script: &str) -> Option<u16> {
    let tokens: Vec<&str> = script.split_whitespace().collect();
    for (i, token) in tokens.iter().enumerate() {
        if *token == "--port" || *token == "-p" {
            if let Some(next) = tokens.get(i + 1) {
                if let Ok(p) = next.parse::<u16>() {
                    return Some(p);
                }
            }
        }
        if let Some(p) = token.strip_prefix("--port=") {
            if let Ok(p) = p.parse::<u16>() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guess_port_from_script() {
        assert_eq!(guess_port("next dev --port 3001"), Some(3001));
        assert_eq!(guess_port("vite --port=5174"), Some(5174));
        assert_eq!(guess_port("vite"), None);
    }

    #[test]
    fn dev_command_per_pm() {
        assert_eq!(dev_command("pnpm"), "pnpm dev");
        assert_eq!(dev_command("npm"), "npm run dev");
    }
}
