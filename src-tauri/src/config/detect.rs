use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePackage {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandCandidate {
    pub label: String,
    pub cwd: String,
    pub command: String,
    pub port: Option<u16>,
    pub reason: String,
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectResult {
    pub name: Option<String>,
    pub command: String,
    pub package_manager: String,
    pub port: Option<u16>,
    pub dev_script: Option<String>,
    pub cwd: String,
    pub candidates: Vec<CommandCandidate>,
    #[serde(default)]
    pub workspace_packages: Vec<WorkspacePackage>,
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
    let pkg = if root.join("package.json").exists() {
        let raw = fs::read_to_string(root.join("package.json"))
            .map_err(|e| format!("无法读取 package.json：{e}"))?;
        serde_json::from_str::<serde_json::Value>(&raw)
            .map_err(|e| format!("package.json 格式错误：{e}"))?
    } else {
        serde_json::Value::Null
    };
    let workspace_packages = list_workspace_packages(&root, &pkg);
    let pm = detect_package_manager(&root);
    let mut candidates = candidates_at(&root);
    let root_count = candidates.len();
    for package in &workspace_packages {
        candidates.extend(candidates_at(Path::new(&package.path)));
    }
    if candidates.is_empty() {
        for child in ["frontend", "web", "client", "backend", "server", "api"] {
            candidates.extend(candidates_at(&root.join(child)));
        }
    }
    if candidates.is_empty() {
        return Err(
            "未找到可识别的 dev/start/serve、Django 或 Rust 入口，请手动填写启动命令".into(),
        );
    }
    let default = if root_count > 0 {
        candidates.first()
    } else if candidates.len() == 1 {
        candidates.first()
    } else {
        None
    };
    Ok(DetectResult {
        name: pkg["name"]
            .as_str()
            .map(humanize_name)
            .or_else(|| root.file_name().map(|s| s.to_string_lossy().into_owned())),
        command: default.map(|c| c.command.clone()).unwrap_or_default(),
        cwd: default
            .map(|c| c.cwd.clone())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        package_manager: pm.into(),
        port: default.and_then(|c| c.port),
        dev_script: pkg["scripts"]["dev"].as_str().map(String::from),
        candidates,
        workspace_packages,
    })
}

fn read_package(root: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&fs::read_to_string(root.join("package.json")).ok()?).ok()
}

fn candidates_at(root: &Path) -> Vec<CommandCandidate> {
    let mut out = Vec::new();
    let cwd = root.to_string_lossy().into_owned();
    if let Some(pkg) = read_package(root) {
        let pm = detect_package_manager(root);
        let name = pkg["name"].as_str().unwrap_or("项目");
        // Desktop and web are separate, reviewable choices.
        if root.join("src-tauri").is_dir() && pkg["scripts"]["tauri"].is_string() {
            out.push(CommandCandidate {
                label: format!("{name} · 桌面开发"),
                cwd: cwd.clone(),
                command: tauri_dev_command(pm),
                port: detect_tauri_port(root),
                reason: "src-tauri + scripts.tauri".into(),
                kind: "desktop",
            });
        }
        if let Some(scripts) = pkg["scripts"].as_object() {
            let mut names: Vec<_> = scripts
                .keys()
                .filter(|s| {
                    matches!(s.as_str(), "dev" | "start" | "serve") || s.starts_with("dev:")
                })
                .filter(|s| {
                    s.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_:-".contains(c))
                })
                .collect();
            names.sort_by_key(|s| {
                (
                    match s.as_str() {
                        "dev" => 0,
                        "start" => 1,
                        "serve" => 2,
                        _ => 3,
                    },
                    s.as_str(),
                )
            });
            for script in names {
                let Some(value) = scripts[script].as_str() else {
                    continue;
                };
                out.push(CommandCandidate {
                    label: format!("{name} · {script}"),
                    cwd: cwd.clone(),
                    command: format!("{pm} run {script}"),
                    port: guess_port(value).or_else(|| {
                        if script == "dev" {
                            detect_tauri_port(root)
                        } else {
                            None
                        }
                    }),
                    reason: format!("package.json scripts.{script} · {pm} · {value}"),
                    kind: classify_script(root, &pkg, value),
                });
            }
        }
    } else if root.join("manage.py").is_file() {
        let python = if root.join(".venv/Scripts/python.exe").is_file() {
            ".venv\\Scripts\\python.exe"
        } else {
            "python"
        };
        out.push(CommandCandidate {
            label: "Django 开发服务".into(),
            cwd,
            command: format!("{python} manage.py runserver"),
            port: Some(8000),
            reason: "manage.py；请确认 Python 环境已安装依赖".into(),
            kind: "backend",
        });
    } else if root.join("Cargo.toml").is_file() && root.join("src/main.rs").is_file() {
        out.push(CommandCandidate {
            label: "Rust 程序".into(),
            cwd,
            command: "cargo run".into(),
            port: None,
            reason: "Cargo.toml + src/main.rs".into(),
            kind: "unknown",
        });
    }
    out
}

fn classify_script(root: &Path, pkg: &serde_json::Value, script: &str) -> &'static str {
    let script = script.to_lowercase();
    if script.contains("electron") || script.contains("tauri") { return "desktop"; }
    if ["nest ", "nodemon", "ts-node", "tsx ", "uvicorn", "manage.py", "node server", "node dist/main", "node src/server"].iter().any(|s| script.contains(s)) { return "backend"; }
    if ["vite", "next ", "nuxt", "react-scripts", "vue-cli-service", "astro", "webpack serve"].iter().any(|s| script.contains(s)) { return "frontend"; }
    let has = |name: &str| pkg["dependencies"][name].is_string() || pkg["devDependencies"][name].is_string();
    if ["@nestjs/core", "express", "fastify", "koa"].iter().any(|name| has(name)) { return "backend"; }
    let directory = root.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    if ["api", "backend", "server"].contains(&directory.as_str()) { return "backend"; }
    if ["web", "frontend", "website", "client"].contains(&directory.as_str()) { return "frontend"; }
    "unknown"
}

#[cfg(test)]
#[test]
fn candidate_classification_uses_script_content_and_keeps_unknown_explicit() {
    let package=serde_json::json!({"scripts":{"dev":"electron-vite dev"}});
    assert_eq!(classify_script(Path::new("D:/project"),&package,"electron-vite dev"),"desktop");
    assert_eq!(classify_script(Path::new("D:/project"),&package,"vite --port 5173"),"frontend");
    assert_eq!(classify_script(Path::new("D:/project"),&package,"nest start --watch"),"backend");
    assert_eq!(classify_script(Path::new("D:/project"),&package,"custom launcher"),"unknown");
}

fn list_workspace_packages(root: &Path, pkg: &serde_json::Value) -> Vec<WorkspacePackage> {
    let mut globs: Vec<String> = Vec::new();

    if let Some(arr) = pkg["workspaces"].as_array() {
        for v in arr {
            if let Some(s) = v.as_str() {
                globs.push(s.to_string());
            }
        }
    } else if let Some(arr) = pkg["workspaces"]["packages"].as_array() {
        for v in arr {
            if let Some(s) = v.as_str() {
                globs.push(s.to_string());
            }
        }
    }

    if let Ok(raw) = fs::read_to_string(root.join("pnpm-workspace.yaml")) {
        for line in raw.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("- ") {
                let g = rest.trim().trim_matches('\'').trim_matches('"');
                if !g.is_empty() && !g.starts_with('#') {
                    globs.push(g.to_string());
                }
            }
        }
    }

    if globs.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for pattern in globs {
        for dir in expand_workspace_glob(root, &pattern) {
            let pkg_json = dir.join("package.json");
            if !pkg_json.exists() {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&pkg_json) else {
                continue;
            };
            let Ok(sub) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            if candidates_at(&dir).is_empty() {
                continue;
            }
            let path = dir.to_string_lossy().into_owned();
            if !seen.insert(path.clone()) {
                continue;
            }
            let name = sub["name"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    dir.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("pkg")
                        .into()
                });
            out.push(WorkspacePackage { name, path });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn expand_workspace_glob(root: &Path, pattern: &str) -> Vec<PathBuf> {
    // ponytail: only `packages/*` / `apps/*` style; nested ** not needed yet
    let pattern = pattern.replace('\\', "/");
    if let Some(prefix) = pattern.strip_suffix("/*") {
        let base = root.join(prefix);
        let Ok(rd) = fs::read_dir(&base) else {
            return Vec::new();
        };
        return rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect();
    }
    let direct = root.join(&pattern);
    if direct.is_dir() {
        vec![direct]
    } else {
        Vec::new()
    }
}

fn detect_package_manager(root: &Path) -> &'static str {
    for (depth, dir) in root.ancestors().take(5).enumerate() {
        if depth > 0
            && !dir.join("pnpm-workspace.yaml").exists()
            && !read_package(dir)
                .is_some_and(|p| p["workspaces"].is_array() || p["workspaces"].is_object())
        {
            continue;
        }
        if let Some(pkg) = read_package(dir) {
            if let Some(explicit) = pkg["packageManager"]
                .as_str()
                .and_then(|v| v.split('@').next())
            {
                match explicit {
                    "npm" => return "npm",
                    "pnpm" => return "pnpm",
                    "yarn" => return "yarn",
                    "bun" => return "bun",
                    _ => {}
                }
            }
        }
        if dir.join("pnpm-lock.yaml").exists() || dir.join("pnpm-workspace.yaml").exists() {
            return "pnpm";
        }
        if dir.join("yarn.lock").exists() {
            return "yarn";
        }
        if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
            return "bun";
        }
        if dir.join("package-lock.json").exists() {
            return "npm";
        }
    }
    "npm"
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

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "qd-detect-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn package(&self, child: &str, raw: &str) {
            let dir = self.0.join(child);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("package.json"), raw).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn workspace_uses_inherited_manager_and_never_picks_arbitrary_child() {
        let f = Fixture::new();
        f.package(
            "",
            r#"{"name":"suite","packageManager":"pnpm@9.0.0","workspaces":["apps/*"]}"#,
        );
        f.package(
            "apps/a",
            r#"{"name":"a","scripts":{"dev":"vite --port 3111"}}"#,
        );
        f.package("apps/b", r#"{"name":"b","scripts":{"start":"next start"}}"#);
        let result = detect_project(f.0.to_str().unwrap()).unwrap();
        assert!(result.command.is_empty());
        assert_eq!(result.candidates.len(), 2);
        assert!(result
            .candidates
            .iter()
            .all(|c| c.command.starts_with("pnpm run ")));
        assert!(result
            .candidates
            .iter()
            .all(|c| Path::new(&c.cwd).join("package.json").exists()));
    }

    #[test]
    fn desktop_and_web_are_distinct_choices_and_tauri_directory_resolves() {
        let f = Fixture::new();
        f.package(
            "",
            r#"{"packageManager":"bun@1.0","scripts":{"dev":"vite","tauri":"tauri"}}"#,
        );
        fs::create_dir_all(f.0.join("src-tauri")).unwrap();
        let result = detect_project(f.0.join("src-tauri").to_str().unwrap()).unwrap();
        assert_eq!(result.command, "bun run tauri dev");
        assert_eq!(result.candidates[1].command, "bun run dev");
        assert_eq!(Path::new(&result.cwd), f.0);
    }

    #[test]
    fn discovers_common_child_with_correct_execution_directory() {
        let f = Fixture::new();
        f.package("web", r#"{"scripts":{"serve":"vite --port=5174"}}"#);
        let result = detect_project(f.0.to_str().unwrap()).unwrap();
        assert_eq!(Path::new(&result.cwd), f.0.join("web"));
        assert_eq!(result.port, Some(5174));
        assert_eq!(result.command, "npm run serve");
    }

    #[test]
    fn expand_star_glob() {
        let tmp = std::env::temp_dir().join(format!(
            "qd-ws-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(tmp.join("packages/a")).unwrap();
        fs::create_dir_all(tmp.join("packages/b")).unwrap();
        let dirs = expand_workspace_glob(&tmp, "packages/*");
        assert_eq!(dirs.len(), 2);
        let _ = fs::remove_dir_all(&tmp);
    }
}
