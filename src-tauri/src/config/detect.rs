use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
#[path = "vite.rs"]
mod vite;
#[path = "scan.rs"]
mod scan;

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
    pub open_url: Option<String>,
    pub warning: Option<String>,
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
    pub warnings: Vec<String>,
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
    let mut budget = scan::Budget::default();
    let mut warnings = Vec::new();
    let workspace_packages = list_workspace_packages(&root, &pkg, &mut budget, &mut warnings);
    let pm = detect_package_manager(&root);
    let (directories, scan_warnings) = scan::discover(&root, workspace_packages.iter().map(|package| PathBuf::from(&package.path)), &mut budget);
    warnings.extend(scan_warnings);
    warnings.sort();
    warnings.dedup();
    let mut candidates = Vec::new();
    let mut root_count = 0;
    let mut seen = std::collections::HashSet::new();
    for directory in directories {
        let found = candidates_at(&directory);
        if directory == root { root_count = found.len(); }
        for candidate in found {
            let identity = (directory.canonicalize().unwrap_or(directory.clone()).to_string_lossy().to_lowercase(), candidate.command.clone());
            if seen.insert(identity) { candidates.push(candidate); }
        }
    }
    if candidates.is_empty() {
        return Err(
            format!("未找到可识别的 dev/start/serve、Django 或 Rust 入口（扫描项目内最多 {} 层，跳过依赖、构建目录和链接目录）；请选择具体服务目录或手动填写启动命令。{}",scan::MAX_DEPTH,warnings.join("；")),
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
        warnings,
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
                open_url: None,
                warning: None,
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
                let explicit_port = guess_port(value);
                let vite_address = if value.split_whitespace().next() == Some("vite") {
                    Some(if value.split_whitespace().nth(1).is_some_and(|token| token=="preview" || token=="build") {
                        Err("Vite 命令不是开发服务；请确认并填写网页端口或地址".into())
                    } else if value.split_whitespace().any(|token| token=="--config" || token=="-c" || token.starts_with("--config=")) {
                        Err("Vite 使用自定义配置路径；请填写网页端口或地址".into())
                    } else if value.split_whitespace().any(|token| token=="--host" || token.starts_with("--host=")) {
                        Err("Vite 命令覆盖了 host；请确认并填写网页端口或地址".into())
                    } else { vite::read_address(root) })
                } else { None };
                let inferred_port = explicit_port.or_else(|| vite_address.as_ref().and_then(|result| result.as_ref().ok().map(|(port,_)| *port)));
                let open_url = vite_address.as_ref().and_then(|result| result.as_ref().ok()).map(|(_,url)| {
                    if let Some(port)=explicit_port { format!("{}:{port}/",url.rsplit_once(':').unwrap().0) } else { url.clone() }
                });
                let warning = vite_address.as_ref().and_then(|result| result.as_ref().err()).cloned();
                out.push(CommandCandidate {
                    label: format!("{name} · {script}"),
                    cwd: cwd.clone(),
                    command: format!("{pm} run {script}"),
                    port: inferred_port.or_else(|| {
                        if script == "dev" {
                            detect_tauri_port(root)
                        } else {
                            None
                        }
                    }),
                    open_url,
                    warning,
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
            open_url: None,
            warning: None,
            reason: "manage.py；请确认 Python 环境已安装依赖".into(),
            kind: "backend",
        });
    } else if root.join("Cargo.toml").is_file() && root.join("src/main.rs").is_file() {
        out.push(CommandCandidate {
            label: "Rust 程序".into(),
            cwd,
            command: "cargo run".into(),
            port: None,
            open_url: None,
            warning: None,
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

fn list_workspace_packages(root: &Path, pkg: &serde_json::Value, budget: &mut scan::Budget, warnings: &mut Vec<String>) -> Vec<WorkspacePackage> {
    let mut globs: Vec<String> = Vec::new();

    if let Some(arr) = pkg["workspaces"].as_array() {
        for v in arr.iter().take(scan::MAX_DIRECTORIES) {
            if let Some(s) = v.as_str() {
                globs.push(s.to_string());
            }
        }
    } else if let Some(arr) = pkg["workspaces"]["packages"].as_array() {
        for v in arr.iter().take(scan::MAX_DIRECTORIES) {
            if let Some(s) = v.as_str() {
                globs.push(s.to_string());
            }
        }
    }

    if let Ok(raw) = fs::read_to_string(root.join("pnpm-workspace.yaml")) {
        for line in raw.lines().take(scan::MAX_DIRECTORIES) {
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
    if globs.len() >= scan::MAX_DIRECTORIES {
        warnings.push("workspace 声明最多检查 128 项；其余服务可选择具体目录或手动添加".into());
    }
    for pattern in globs.into_iter().take(scan::MAX_DIRECTORIES) {
        for dir in expand_workspace_glob(root, &pattern, budget, warnings) {
            if out.len() >= scan::MAX_DIRECTORIES { break; }
            if !scan::within_scope(root,&dir) { continue; }
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
            if !seen.insert(dir.canonicalize().unwrap_or(dir.clone()).to_string_lossy().to_lowercase()) {
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

fn expand_workspace_glob(root: &Path, pattern: &str, budget: &mut scan::Budget, warnings: &mut Vec<String>) -> Vec<PathBuf> {
    // ponytail: only `packages/*` / `apps/*` style; nested ** not needed yet
    let pattern = pattern.replace('\\', "/");
    if pattern.starts_with('/') || pattern.contains(':') || pattern.split('/').any(|part| part=="..") { return Vec::new(); }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        let base = root.join(prefix);
        if !scan::within_scope(root,&base) { return Vec::new(); }
        let Ok(rd) = fs::read_dir(&base) else {
            return Vec::new();
        };
        let mut directories = Vec::new();
        for entry in rd {
            if !budget.take_entry() { warnings.push(scan::entry_limit_warning()); break; }
            let Ok(entry) = entry else { continue; };
            if scan::within_scope(root,&entry.path()) { directories.push(entry.path()); }
            if directories.len() >= scan::MAX_DIRECTORIES { break; }
        }
        directories.sort();
        return directories;
    }
    let direct = root.join(&pattern);
    if !budget.take_entry() { warnings.push(scan::entry_limit_warning()); return Vec::new(); }
    if scan::within_scope(root,&direct) {
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

    #[test]
    fn vite_candidate_reads_literal_address_and_reports_dynamic_config() {
        let f = Fixture::new();
        f.package("", r#"{"scripts":{"dev":"vite"}}"#);
        let config = f.0.join("vite.config.ts");
        fs::write(&config,"import {execSync} from 'node:child_process'; export default defineConfig({server:{host:'127.0.0.2',port:6123}});").unwrap();
        let result=detect_project(f.0.to_str().unwrap()).unwrap();
        assert_eq!(result.candidates[0].port,Some(6123));
        assert_eq!(result.candidates[0].open_url.as_deref(),Some("http://127.0.0.2:6123/"));
        assert!(result.candidates[0].warning.is_none());
        fs::write(&config,"export default defineConfig(()=>({server:{port:process.env.PORT}}))").unwrap();
        let result=detect_project(f.0.to_str().unwrap()).unwrap();
        assert!(result.candidates[0].port.is_none());
        assert!(result.candidates[0].warning.as_deref().is_some_and(|warning| warning.contains("无法静态识别")));
        f.package("", r#"{"scripts":{"dev":"vite --port 4321"}}"#);
        assert_eq!(detect_project(f.0.to_str().unwrap()).unwrap().candidates[0].port,Some(4321));
        fs::write(&config,"export default {server:{host:'127.0.0.2',port:6123}}").unwrap();
        assert_eq!(detect_project(f.0.to_str().unwrap()).unwrap().candidates[0].open_url.as_deref(),Some("http://127.0.0.2:4321/"));
        f.package("", r#"{"scripts":{"dev":"vite preview"}}"#);
        assert!(detect_project(f.0.to_str().unwrap()).unwrap().candidates[0].port.is_none());
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
    fn nested_layouts_keep_service_directories_and_static_vite_address() {
        for (web, api) in [("app/web", "app/api"), ("app/frontend", "app/backend")] {
            let f = Fixture::new();
            f.package(web, r#"{"scripts":{"dev":"vite"}}"#);
            fs::write(f.0.join(web).join("vite.config.ts"), "export default {server:{host:'127.0.0.1',port:5174,strictPort:true}}").unwrap();
            fs::create_dir_all(f.0.join(api)).unwrap();
            fs::write(f.0.join(api).join("manage.py"), "raise RuntimeError('must never execute')").unwrap();
            let result = detect_project(f.0.to_str().unwrap()).unwrap();
            assert_eq!(result.candidates.len(), 2);
            let frontend = result.candidates.iter().find(|c| c.kind == "frontend").unwrap();
            assert_eq!(Path::new(&frontend.cwd), f.0.join(web));
            assert_eq!(frontend.open_url.as_deref(), Some("http://127.0.0.1:5174/"));
            let backend = result.candidates.iter().find(|c| c.kind == "backend").unwrap();
            assert_eq!(Path::new(&backend.cwd), f.0.join(api));
            assert_eq!(backend.port, Some(8000));
            assert!(result.command.is_empty());
        }
    }

    #[test]
    fn root_entry_and_workspace_aliases_do_not_hide_or_duplicate_children() {
        let f = Fixture::new();
        f.package("", r#"{"scripts":{"dev":"vite --port 4000"},"workspaces":[".","app/*","app/web","app/./web"]}"#);
        f.package("app/web", r#"{"scripts":{"dev":"vite --port 5174"}}"#);
        fs::create_dir_all(f.0.join("app/api")).unwrap();
        fs::write(f.0.join("app/api/manage.py"), "").unwrap();
        let first = detect_project(f.0.to_str().unwrap()).unwrap();
        let second = detect_project(f.0.to_str().unwrap()).unwrap();
        assert_eq!(first.candidates.len(), 3);
        assert_eq!(first.command, "npm run dev");
        assert_eq!(first.port, Some(4000));
        assert_eq!(first.workspace_packages.len(), 2);
        assert_eq!(serde_json::to_value(first).unwrap(), serde_json::to_value(second).unwrap());
    }

    #[test]
    fn excludes_dependencies_outputs_hidden_paths_and_fourth_level() {
        let f = Fixture::new();
        f.package("one/two/web", r#"{"scripts":{"dev":"vite"}}"#);
        for dir in ["node_modules/trap", "app/node_modules/trap", "target/trap", "dist/trap", ".hidden/trap", "one/two/three/web", "src-tauri/trap", "app/build/trap"] {
            f.package(dir, r#"{"scripts":{"dev":"vite"}}"#);
        }
        let result = detect_project(f.0.to_str().unwrap()).unwrap();
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(Path::new(&result.candidates[0].cwd), f.0.join("one/two/web"));
    }

    #[test]
    fn workspace_paths_cannot_escape_selected_root() {
        let f = Fixture::new();
        let outside = Fixture::new();
        outside.package("web", r#"{"scripts":{"dev":"vite"}}"#);
        f.package("app/web", r#"{"scripts":{"dev":"vite"}}"#);
        let relative = format!("../{}/web", outside.0.file_name().unwrap().to_string_lossy());
        let pkg = serde_json::json!({"workspaces":[relative, outside.0.join("web").to_string_lossy(), "app/web", "app/../app/web", "node_modules/*"]});
        f.package("", &pkg.to_string());
        let result = detect_project(f.0.to_str().unwrap()).unwrap();
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.workspace_packages.len(), 1);
        assert_eq!(Path::new(&result.workspace_packages[0].path), f.0.join("app/web"));
    }

    #[test]
    fn directory_and_shared_workspace_entry_budgets_report_truncation() {
        let f = Fixture::new();
        f.package("", r#"{"scripts":{"dev":"vite"}}"#);
        for index in 0..140 { f.package(&format!("app{index:03}"), r#"{"scripts":{"dev":"vite"}}"#); }
        let result = detect_project(f.0.to_str().unwrap()).unwrap();
        assert_eq!(result.candidates.len(), scan::MAX_DIRECTORIES);
        assert!(result.warnings.iter().any(|warning| warning.contains("128 个目录")));

        let crowded = Fixture::new();
        crowded.package("", r#"{"scripts":{"dev":"vite"}}"#);
        for index in 0..4097 { fs::write(crowded.0.join(format!("file{index:04}.txt")), "").unwrap(); }
        let limited = detect_project(crowded.0.to_str().unwrap()).unwrap();
        assert_eq!(limited.candidates.len(), 1);
        assert!(limited.warnings.iter().any(|warning| warning.contains("4096")));

        let mut budget = scan::Budget::default();
        for _ in 0..4096 { assert!(budget.take_entry()); }
        let mut warnings = Vec::new();
        assert!(expand_workspace_glob(&f.0, "app000", &mut budget, &mut warnings).is_empty());
        let (directories, scan_warnings) = scan::discover(&f.0, std::iter::empty(), &mut budget);
        assert_eq!(directories, vec![f.0.clone()]);
        assert!(warnings[0].contains("4096"));
        assert!(scan_warnings[0].contains("4096"));
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_skipped_for_scan_and_workspace_even_when_they_point_inside() {
        let f = Fixture::new();
        let outside = Fixture::new();
        f.package("app/web", r#"{"scripts":{"dev":"vite"}}"#);
        outside.package("", r#"{"scripts":{"dev":"vite"}}"#);
        for (name, target) in [("linked-outside", outside.0.clone()), ("linked-inside", f.0.join("app")), ("loop", f.0.clone())] {
            let link = f.0.join(name);
            let output = std::process::Command::new("cmd.exe").args(["/D", "/C", "mklink", "/J"]).arg(&link).arg(&target).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        }
        f.package("", r#"{"workspaces":["linked-outside","linked-inside/*","loop/*","app/*"]}"#);
        let result = detect_project(f.0.to_str().unwrap()).unwrap();
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.workspace_packages.len(), 1);
        for name in ["linked-outside", "linked-inside", "loop"] { fs::remove_dir(f.0.join(name)).unwrap(); }
        assert!(outside.0.join("package.json").exists());
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
        let dirs = expand_workspace_glob(&tmp, "packages/*", &mut scan::Budget::default(), &mut Vec::new());
        assert_eq!(dirs.len(), 2);
        let _ = fs::remove_dir_all(&tmp);
    }
}
