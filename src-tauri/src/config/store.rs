use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::sync::Mutex;

static WRITE_LOCK: Mutex<()> = Mutex::new(());
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub command: String,
    pub port: Option<u16>,
    pub open_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub services: Vec<ProjectService>,
    #[serde(default)]
    pub start_mode: StartMode,
    #[serde(default)]
    pub launch_plans: Vec<LaunchPlan>,
    #[serde(default)]
    pub default_plan_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPlan {
    pub id: String,
    pub name: String,
    pub service_ids: Vec<String>,
    #[serde(default)]
    pub start_mode: StartMode,
    #[serde(default)]
    pub open_service_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StartMode { #[default] Parallel, Sequential }

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Readiness { #[default] Auto, Process, Port, Http, Window }

fn default_timeout() -> u64 { 60 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectService {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    pub cwd: String,
    pub command: String,
    pub port: Option<u16>,
    pub open_url: Option<String>,
    #[serde(default)]
    pub readiness: Readiness,
    #[serde(default)]
    pub health_url: Option<String>,
    #[serde(default = "default_timeout")]
    pub startup_timeout_secs: u64,
}

impl Project {
    pub fn effective_services(&self) -> Vec<ProjectService> {
        if !self.services.is_empty() {
            return self.services.iter().enumerate().map(|(index, service)| {
                let mut service = service.clone();
                if service.name.trim().is_empty() { service.name = format!("任务 {}", index + 1); }
                service
            }).collect();
        }
        vec![ProjectService { kind: None, id: "default".into(), name: "服务".into(), cwd: self.cwd.clone(), command: self.command.clone(), port: self.port, open_url: self.open_url.clone(), readiness: Readiness::Auto, health_url: None, startup_timeout_secs: default_timeout() }]
    }
    pub fn normalize(mut self) -> Self {
        if self.default_plan_id.is_none() { self.default_plan_id=self.launch_plans.first().map(|p|p.id.clone()); }
        self.services = self.effective_services();
        if let Some(first) = self.services.first() {
            self.cwd = first.cwd.clone(); self.command = first.command.clone(); self.port = first.port; self.open_url = first.open_url.clone();
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_true")]
    pub tray_animation: bool,
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
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u32,
    #[serde(default = "default_start_page")]
    pub start_page: String,
}

fn default_refresh_interval() -> u32 { 5 }
fn default_start_page() -> String { "ports".into() }

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
            tray_animation: true,
            highlight_ports: default_highlight_ports(),
            auto_refresh: true,
            minimize_to_tray: true,
            auto_start: false,
            hotkey: default_hotkey(),
            write_logs: false,
            groups: default_groups(),
            shell: default_shell(),
            refresh_interval_secs: default_refresh_interval(),
            start_page: default_start_page(),
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
        version: 2,
        projects: get_projects()?,
        settings: get_settings()?,
    };
    serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())
}

pub fn import_all(json: &str) -> Result<(), String> {
    let _guard = WRITE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut bundle: ConfigBundle = serde_json::from_str(json).map_err(|e| e.to_string())?;
    validate_bundle(&bundle)?;
    bundle.projects = bundle.projects.into_iter().map(Project::normalize).collect();
    let projects_path = data_dir()?.join("projects.json");
    backup_legacy_projects(&projects_path)?;
    let previous_settings = get_settings()?;
    save_settings_inner(bundle.settings)?;
    let projects_json =
        serde_json::to_string_pretty(&bundle.projects).map_err(|e| e.to_string())?;
    if let Err(e) = atomic_write(&projects_path, &projects_json) {
        save_settings_inner(previous_settings)?;
        return Err(e);
    }
    Ok(())
}

pub fn validate_project(project: &Project) -> Result<(), String> {
    if project.id.is_empty()
        || project.id == "."
        || project.id == ".."
        || project
            .id
            .chars()
            .any(|c| c.is_control() || "<>:\\|?*/\"".contains(c))
    {
        return Err("项目 ID 不能含路径或文件名非法字符".into());
    }
    if project.name.trim().is_empty() { return Err("项目名称不能为空".into()); }
    let mut service_ids = HashSet::new();
    let mut ports = HashSet::new();
    for service in project.effective_services() {
        if service.id.is_empty() || service.id.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_') || !service_ids.insert(service.id.clone()) { return Err("服务 ID 必须唯一，且仅含字母、数字、横线或下划线".into()); }
        if service.cwd.trim().is_empty() || service.command.trim().is_empty() { return Err("任务目录和启动命令不能为空".into()); }
        if service.kind.as_deref().is_some_and(|kind| !["frontend","backend","desktop","unknown"].contains(&kind)) {return Err("服务分类无效".into());}
        if service.port == Some(0) || project.launch_plans.is_empty() && service.port.is_some_and(|p| !ports.insert(p)) { return Err("服务端口必须为 1–65535，联合启动的服务不能使用相同端口".into()); }
        if !(1..=3600).contains(&service.startup_timeout_secs) { return Err("启动超时必须为 1–3600 秒".into()); }
        if service.readiness == Readiness::Port && service.port.is_none() { return Err("端口就绪检查需要预期端口".into()); }
        for url in [&service.open_url, &service.health_url].into_iter().flatten() {
            let parsed = url.parse::<tauri::Url>().map_err(|_| "地址格式不正确")?;
            if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() { return Err("地址必须使用 http:// 或 https://".into()); }
        }
        if service.readiness == Readiness::Http && service.health_url.as_ref().is_none_or(|url| url.trim().is_empty()) { return Err("HTTP 就绪检查需要健康地址".into()); }
        if service.readiness == Readiness::Http {
            let url = service.health_url.as_ref().unwrap().parse::<tauri::Url>().map_err(|_| "健康地址格式不正确")?;
            if !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1" | "[::1]")) { return Err("健康检查地址必须指向本机 localhost、127.0.0.1 或 ::1".into()); }
        }
    }
    let mut plan_ids = HashSet::new();
    for plan in &project.launch_plans {
        if plan.id.is_empty() || plan.id.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_') || !plan_ids.insert(&plan.id) || plan.name.trim().is_empty() { return Err("启动方案名称或 ID 无效".into()); }
        let mut members = HashSet::new();
        if plan.service_ids.is_empty() || plan.service_ids.iter().any(|id| !service_ids.contains(id) || !members.insert(id)) { return Err("启动方案需包含有效且不重复的服务".into()); }
        if plan.open_service_id.as_ref().is_some_and(|id| !plan.service_ids.contains(id)) { return Err("打开入口必须属于该启动方案".into()); }
        let mut plan_ports=HashSet::new();
        if project.effective_services().iter().filter(|s|plan.service_ids.contains(&s.id)).filter_map(|s|s.port).any(|port|!plan_ports.insert(port)) { return Err("同一启动方案中服务端口不能重复".into()); }
    }
    if project.default_plan_id.as_ref().is_some_and(|id| !plan_ids.contains(id)) { return Err("默认启动方案不存在".into()); }
    Ok(())
}

fn validate_bundle(bundle: &ConfigBundle) -> Result<(), String> {
    if !matches!(bundle.version, 1 | 2) {
        return Err("不支持的配置版本".into());
    }
    let mut ids = HashSet::new();
    for project in &bundle.projects {
        validate_project(project)?;
        if !ids.insert(&project.id) {
            return Err("配置中存在重复项目 ID".into());
        }
    }
    validate_settings(&bundle.settings)
}

fn validate_settings(settings: &Settings) -> Result<(), String> {
    if ![3,5,10,30].contains(&settings.refresh_interval_secs) {
        return Err("刷新间隔请选择 3、5、10 或 30 秒".into());
    }
    if !["ports","projects","settings"].contains(&settings.start_page.as_str()) {
        return Err("默认首页无效".into());
    }
    if settings.highlight_ports.contains(&0) {
        return Err("高亮端口必须为 1–65535".into());
    }
    if settings.shell.trim().is_empty() {
        return Err("Shell 不能为空".into());
    }
    Ok(())
}

pub(crate) fn data_dir() -> Result<PathBuf, String> {
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
    let projects: Vec<Project> = read_json(&path, Vec::new())?;
    Ok(projects.into_iter().map(Project::normalize).collect())
}

fn backup_legacy_projects(path: &PathBuf) -> Result<(), String> {
    if !path.exists() { return Ok(()); }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let original: Vec<serde_json::Value> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if original.iter().any(|p| p.get("services").is_none()) {
        let backup = path.with_file_name("projects.v1.backup.json");
        if !backup.exists() { atomic_write(&backup, &raw)?; }
    }
    Ok(())
}

pub fn save_project(project: Project) -> Result<(), String> {
    validate_project(&project)?;
    let project = project.normalize();
    let _guard = WRITE_LOCK.lock().map_err(|e| e.to_string())?;
    let path = data_dir()?.join("projects.json");
    backup_legacy_projects(&path)?;
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
    let _guard = WRITE_LOCK.lock().map_err(|e| e.to_string())?;
    let path = data_dir()?.join("projects.json");
    backup_legacy_projects(&path)?;
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
    let _guard = WRITE_LOCK.lock().map_err(|e| e.to_string())?;
    save_settings_inner(settings)
}

fn save_settings_inner(settings: Settings) -> Result<(), String> {
    validate_settings(&settings)?;
    let path = data_dir()?.join("settings.json");
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    atomic_write(&path, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_ids_and_duplicate_imports_before_writing() {
        let project = Project {
            id: "../outside".into(),
            name: "test".into(),
            cwd: "D:/test".into(),
            command: "npm run dev".into(),
            port: Some(3000),
            open_url: None,
            tags: vec![],
            group: None,
            services: vec![],
            start_mode: StartMode::Parallel, launch_plans: vec![], default_plan_id: None,
        };
        assert!(validate_project(&project).is_err());
        let valid = Project {
            id: "test".into(),
            ..project
        };
        let bundle = ConfigBundle {
            version: 1,
            projects: vec![valid.clone(), valid],
            settings: Settings::default(),
        };
        assert!(validate_bundle(&bundle).is_err());
    }

    #[test]
    fn rejects_unknown_import_version_and_zero_ports() {
        let mut bundle = ConfigBundle {
            version: 999,
            projects: vec![],
            settings: Settings::default(),
        };
        assert!(validate_bundle(&bundle).is_err());
        bundle.version = 1;
        bundle.settings.highlight_ports = vec![0];
        assert!(validate_bundle(&bundle).is_err());
    }

    #[test]
    fn legacy_project_round_trip_preserves_fields_and_multi_service_validation() {
        let old=r#"{"id":"legacy","name":"旧项目","cwd":"D:/中文 space","command":"pnpm dev","port":5173,"openUrl":"http://localhost:5173","group":"工作","tags":["web"]}"#;
        let project:Project=serde_json::from_str(old).unwrap();let mut project=project.normalize();
        assert_eq!(project.services.len(),1);assert_eq!(project.services[0].id,"default");assert_eq!(project.services[0].startup_timeout_secs,60);
        assert_eq!(project.group.as_deref(),Some("工作"));assert_eq!(project.tags,vec!["web"]);
        validate_project(&project).unwrap();
        let encoded=serde_json::to_string(&project).unwrap();let decoded:Project=serde_json::from_str(&encoded).unwrap();assert_eq!(decoded.services[0].command,"pnpm dev");
        project.services.push(project.services[0].clone());assert!(validate_project(&project).is_err());
        project.services[1].id="api".into();assert!(validate_project(&project).is_err());
        project.services[1].port=Some(8080);project.services[1].readiness=Readiness::Http;project.services[1].health_url=Some("https://example.com/health".into());assert!(validate_project(&project).is_err());
        project.services[1].health_url=Some("http://localhost:8080/health".into());validate_project(&project).unwrap();
        project.services[1].startup_timeout_secs=0;assert!(validate_project(&project).is_err());
    }
    #[test]
    fn legacy_backup_preserves_original_bytes_and_never_overwrites() {
        let temp=std::env::temp_dir().canonicalize().unwrap();
        let root=temp.join(format!("qd-config-backup-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap();let path=root.join("projects.json");let original="[ {\"id\": \"旧项目\", \"command\": \"pnpm dev\"} ]";
        fs::write(&path,original).unwrap();backup_legacy_projects(&path).unwrap();
        let backup=root.join("projects.v1.backup.json");assert_eq!(fs::read_to_string(&backup).unwrap(),original);
        fs::write(&path,"[{\"id\":\"changed\"}]").unwrap();backup_legacy_projects(&path).unwrap();assert_eq!(fs::read_to_string(&backup).unwrap(),original);
        assert_eq!(root.canonicalize().unwrap().parent(),Some(temp.as_path()));fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn settings_default_ports() {
        let s = Settings::default();
        assert!(s.highlight_ports.contains(&3000));
        assert!(s.highlight_ports.contains(&5173));
    }
    #[test]
    fn launch_plan_config_checks_members_default_and_open_target() {
        let mut project:Project=serde_json::from_str(r#"{"id":"plans","name":"项目","services":[{"id":"web","cwd":"D:/web","command":"npm run dev","kind":"frontend"}],"launchPlans":[{"id":"website","name":"官网","serviceIds":["web"],"openServiceId":"web"}],"defaultPlanId":"website"}"#).unwrap();
        validate_project(&project).unwrap();
        let decoded:Project=serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        assert_eq!(decoded.launch_plans[0].service_ids,vec!["web"]);
        assert_eq!(decoded.services[0].kind.as_deref(),Some("frontend"));
        project.launch_plans[0].service_ids.push("missing".into());assert!(validate_project(&project).is_err());
        project.launch_plans[0].service_ids=vec!["web".into()];
        project.launch_plans[0].open_service_id=Some("missing".into());assert!(validate_project(&project).is_err());
        project.launch_plans[0].open_service_id=None;
        project.default_plan_id=Some("missing".into());assert!(validate_project(&project).is_err());
        project.default_plan_id=None;
        project.services[0].port=Some(5173);
        let mut alternative=project.services[0].clone();alternative.id="preview".into();project.services.push(alternative);
        project.launch_plans.push(LaunchPlan{id:"preview".into(),name:"预览".into(),service_ids:vec!["preview".into()],start_mode:StartMode::Parallel,open_service_id:None});
        validate_project(&project).unwrap();
        project.launch_plans[0].service_ids.push("preview".into());assert!(validate_project(&project).is_err());
    }
    #[test]
    fn optional_task_names_and_hotkey_survive_configuration_roundtrip() {
        let mut project: Project = serde_json::from_str(r#"{"id":"optional","name":"项目","services":[{"id":"web","name":"  ","cwd":"D:/web","command":"npm run dev"},{"id":"api","cwd":"D:/api","command":"python app.py"}]}"#).unwrap();
        validate_project(&project).unwrap();
        project = project.normalize();
        assert_eq!(project.services[0].name, "任务 1");
        assert_eq!(project.services[1].name, "任务 2");
        project.services[1].name = "后端".into();
        assert_eq!(project.effective_services()[1].name, "后端");
        project.services[0].command.clear();
        assert!(validate_project(&project).is_err());
        let mut settings = Settings::default();
        settings.hotkey.clear();
        validate_settings(&settings).unwrap();
        let decoded: Settings = serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(decoded.hotkey, "");
    }
    #[test]
    fn older_settings_get_defaults_and_new_preferences_are_validated() {
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.refresh_interval_secs, 5);
        assert_eq!(old.start_page, "ports");
        let mut settings = old;
        settings.refresh_interval_secs = 0;
        assert!(validate_settings(&settings).is_err());
        settings.refresh_interval_secs = 30;
        settings.start_page = "projects".into();
        assert!(validate_settings(&settings).is_ok());
        settings.start_page = "unknown".into();
        assert!(validate_settings(&settings).is_err());
    }
}
