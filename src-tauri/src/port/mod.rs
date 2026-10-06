use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Arc;
use crate::process::job::ProcessJob;
use sysinfo::{Pid, ProcessesToUpdate, ProcessRefreshKind, UpdateKind, System};

#[derive(Clone)]
pub struct RunningProject {
    pub id: String,
    pub name: String,
    pub root_pid: u32,
    pub job: Arc<ProcessJob>,
    pub service_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortEntry {
    pub port: u16,
    pub process: String,
    pub pid: u32,
    pub command: String,
    pub highlight: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
    pub executable: Option<String>,
    pub cwd: Option<String>,
    pub source_app: Option<String>,
    pub source_evidence: Option<String>,
    pub parent_chain: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortSuggest {
    pub cwd: String,
    pub name: Option<String>,
    pub command: String,
    pub port: u16,
    pub pid: u32,
    pub process: String,
}

/// Startup only needs the owner of one port, not the entire process tree.
pub fn find_listening_port(port: u16, owned: &HashSet<u32>) -> Result<Option<PortEntry>, String> {
    let Some(pid) = listening_pids(port)?.into_iter().min() else { return Ok(None); };
    let mut sys = System::new();
    refresh_pids(&mut sys, &HashSet::from([Pid::from_u32(pid)]));
    let (process, command) = process_info(&sys, pid);
    Ok(Some(PortEntry {
        port, process, pid, command, highlight: false, owned: Some(owned.contains(&pid)),
        project_id: None, project_name: None, service_id: None,
        executable: None, cwd: None, source_app: None, source_evidence: None, parent_chain: vec![],
    }))
}

/// Readiness needs fresh listener PIDs, never process metadata or a cached port table.
pub fn listening_pids(port: u16) -> Result<Vec<u32>, String> {
    let sockets = get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP,
    )
    .map_err(|e| e.to_string())?;
    Ok(sockets.into_iter().filter(|s| matches!(&s.protocol_socket_info, ProtocolSocketInfo::Tcp(t) if t.state == TcpState::Listen && t.local_port == port)).flat_map(|s| s.associated_pids).collect())
}

pub fn list_listening_ports(
    highlight_ports: &[u16],
    owned_pids: &HashSet<u32>,
    running_projects: &[RunningProject],
) -> Result<Vec<PortEntry>, String> {
    let sockets = get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP,
    )
    .map_err(|e| e.to_string())?;

    let mut listen: Vec<(u16, u32)> = Vec::new();
    for sock in sockets {
        let ProtocolSocketInfo::Tcp(tcp) = sock.protocol_socket_info else {
            continue;
        };
        if tcp.state != TcpState::Listen {
            continue;
        }
        for pid in sock.associated_pids {
            listen.push((tcp.local_port, pid));
        }
    }

    // ponytail: only refresh socket + project root PIDs (not ProcessesToUpdate::All)
    let mut want: HashSet<Pid> = HashSet::new();
    for &(_, pid) in &listen {
        want.insert(Pid::from_u32(pid));
    }
    for p in running_projects {
        want.insert(Pid::from_u32(p.root_pid));
    }

    let mut sys = System::new();
    refresh_pids(&mut sys, &want);
    // Bound ancestry discovery and refresh only related processes, never all PIDs.
    for _ in 0..16 {
        let parents: HashSet<Pid> = want.iter().filter_map(|pid| sys.process(*pid).and_then(|p| p.parent())).filter(|pid| !want.contains(pid)).collect();
        if parents.is_empty() { break; }
        refresh_pids(&mut sys, &parents);
        want.extend(parents);
    }
    let mut entries = Vec::new();
    let mut seen = HashSet::new();

    for (port, pid) in listen {
        if !seen.insert((port, pid)) {
            continue;
        }
        let highlight = highlight_ports.contains(&port);
        let project = running_projects.iter().find(|p|p.job.contains_pid(pid)).map(|p|(p.id.clone(),p.name.clone(),p.service_id.clone()));
        let owned = if project.is_some() || owned_pids.contains(&pid) {
            Some(true)
        } else if highlight {
            Some(false)
        } else {
            None
        };

        let (process, command) = process_info(&sys, pid);
        let detail = process_source(&sys, pid);
        entries.push(PortEntry {
            port,
            process: project
                .as_ref()
                .map(|(_, name, _)| name.clone())
                .unwrap_or(process),
            pid,
            command,
            highlight,
            owned,
            project_id: project.as_ref().map(|(id, _, _)| id.clone()),
            service_id: project.as_ref().map(|(_, _, service)| service.clone()),
            project_name: project.map(|(_, name, _)| name),
            executable: detail.0, cwd: detail.1, source_app: detail.2, source_evidence: detail.3, parent_chain: detail.4,
        });
    }

    entries.sort_by(|a, b| {
        let a_proj = a.project_name.is_some();
        let b_proj = b.project_name.is_some();
        b_proj
            .cmp(&a_proj)
            .then_with(|| b.highlight.cmp(&a.highlight))
            .then_with(|| a.port.cmp(&b.port))
            .then_with(|| a.pid.cmp(&b.pid))
    });

    Ok(entries)
}

pub fn suggest_from_ports(
    highlight_ports: &[u16],
    known_cwds: &HashSet<String>,
) -> Result<Vec<PortSuggest>, String> {
    let sockets = get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP,
    )
    .map_err(|e| e.to_string())?;

    let mut listen: Vec<(u16, u32)> = Vec::new();
    for sock in sockets {
        let ProtocolSocketInfo::Tcp(tcp) = sock.protocol_socket_info else {
            continue;
        };
        if tcp.state != TcpState::Listen {
            continue;
        }
        if !highlight_ports.is_empty() && !highlight_ports.contains(&tcp.local_port) {
            // still accept common node ports
            let p = tcp.local_port;
            if !(3000..4000).contains(&p) && !(5000..6000).contains(&p) && p != 8080 && p != 4173 {
                continue;
            }
        }
        for pid in sock.associated_pids {
            listen.push((tcp.local_port, pid));
        }
    }

    let mut want: HashSet<Pid> = listen.iter().map(|&(_, p)| Pid::from_u32(p)).collect();
    let mut sys = System::new();
    refresh_pids(&mut sys, &want);
    // refresh parents for cwd fallback
    let mut extra = HashSet::new();
    for pid in want.iter().copied() {
        if let Some(pp) = sys.process(pid).and_then(|p| p.parent()) {
            extra.insert(pp);
        }
    }
    refresh_pids(&mut sys, &extra);
    want.extend(extra);

    let mut out = Vec::new();
    let mut seen_cwd = HashSet::new();

    for (port, pid) in listen {
        let Some(proc) = sys.process(Pid::from_u32(pid)) else {
            continue;
        };
        let name = proc.name().to_string_lossy().to_lowercase();
        let cmd = proc
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        let lower = format!("{name} {cmd}").to_lowercase();
        if !(lower.contains("node")
            || lower.contains("vite")
            || lower.contains("next")
            || lower.contains("bun")
            || lower.contains("deno")
            || lower.contains("python")
            || lower.contains("uvicorn")
            || lower.contains("cargo"))
        {
            continue;
        }

        let cwd = proc
            .cwd()
            .map(|p| p.to_string_lossy().into_owned())
            .filter(|c| !c.is_empty())
            .or_else(|| cwd_from_command(&cmd));

        let Some(cwd) = cwd else { continue };
        let norm = normalize_cwd(&cwd);
        if known_cwds.contains(&norm) || !seen_cwd.insert(norm.clone()) {
            continue;
        }

        let pkg_name = std::fs::read_to_string(std::path::Path::new(&cwd).join("package.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|v| v["name"].as_str().map(|s| s.replace(['-', '_'], " ")));

        let command = if std::path::Path::new(&cwd).join("pnpm-lock.yaml").exists() {
            "pnpm dev".into()
        } else if std::path::Path::new(&cwd).join("yarn.lock").exists() {
            "yarn dev".into()
        } else {
            "npm run dev".into()
        };

        out.push(PortSuggest {
            cwd,
            name: pkg_name,
            command,
            port,
            pid,
            process: proc.name().to_string_lossy().into_owned(),
        });
    }

    out.sort_by(|a, b| a.port.cmp(&b.port));
    Ok(out)
}

fn refresh_pids(sys: &mut System, pids: &HashSet<Pid>) {
    if pids.is_empty() {
        return;
    }
    let list: Vec<Pid> = pids.iter().copied().collect();
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&list), true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_cwd(UpdateKind::OnlyIfNotSet));
}

fn application_name(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "codex.exe" | "codex" => Some("Codex"),
        "code.exe" | "code" => Some("VS Code"),
        "cursor.exe" | "cursor" => Some("Cursor"),
        "windsurf.exe" | "windsurf" => Some("Windsurf"),
        "idea64.exe" => Some("IntelliJ IDEA"),
        "pycharm64.exe" => Some("PyCharm"),
        _ => None,
    }
}

type ProcessSource = (Option<String>, Option<String>, Option<String>, Option<String>, Vec<String>);
fn process_source(sys: &System, pid: u32) -> ProcessSource {
    let Some(process) = sys.process(Pid::from_u32(pid)) else { return (None,None,None,None,vec![]); };
    let executable = process.exe().map(|p| p.to_string_lossy().into_owned());
    let cwd = process.cwd().map(|p| p.to_string_lossy().into_owned()).or_else(|| {
        cwd_from_command(&process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ")).map(|path| format!("推断：{path}"))
    });
    let mut current = process;
    let mut seen = HashSet::from([pid]);
    let mut chain = Vec::new();
    let mut source = application_name(&process.name().to_string_lossy()).map(str::to_owned);
    let mut evidence = source.as_ref().map(|_| format!("监听进程：PID {pid}"));
    for _ in 0..16 {
        let Some(parent_pid) = current.parent() else { break; };
        if !seen.insert(parent_pid.as_u32()) { break; }
        let Some(parent) = sys.process(parent_pid) else { break; };
        // Refuse a reused parent PID that started after its alleged child.
        if parent.start_time() > current.start_time() { break; }
        let name = parent.name().to_string_lossy();
        chain.push(format!("{} · PID {}", name, parent_pid.as_u32()));
        if source.is_none() {
            if let Some(app) = application_name(&name) {
                source = Some(app.into());
                evidence = Some(format!("父进程链：{} · PID {}（来源推断）", name, parent_pid.as_u32()));
            }
        }
        current = parent;
    }
    (executable, cwd, source, evidence, chain)
}

fn process_info(sys: &System, pid: u32) -> (String, String) {
    let Some(proc) = sys.process(Pid::from_u32(pid)) else {
        return ("unknown".into(), String::new());
    };
    let name = proc.name().to_string_lossy().into_owned();
    let command = proc
        .cmd()
        .iter()
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let command = if command.is_empty() {
        name.clone()
    } else {
        command
    };
    (name, command)
}

fn cwd_from_command(cmd: &str) -> Option<String> {
    // Command metadata can omit quotes around an argument containing spaces.
    // Recover only an absolute drive path; never turn its last word into a cwd.
    let lower = cmd.to_ascii_lowercase();
    let idx = lower.find("\\node_modules\\").or_else(|| lower.find("/node_modules/"))?;
    let prefix = &cmd[..idx];
    let start = prefix.match_indices(":\\").chain(prefix.match_indices(":/"))
        .filter_map(|(colon, _)| (colon > 0 && prefix.as_bytes()[colon - 1].is_ascii_alphabetic()).then_some(colon - 1))
        .max()?;
    Some(prefix[start..].to_string())
}

fn normalize_cwd(cwd: &str) -> String {
    let t = cwd.trim_end_matches(['\\', '/']);
    t.replace('/', "\\").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_ports_smoke() {
        let ports = list_listening_ports(&[3000, 5173], &HashSet::new(), &[]).expect("scan");
        assert!(ports.iter().all(|p| p.port > 0));
    }

    #[test]
    fn normalize_slash() {
        assert_eq!(normalize_cwd(r"D:\a\b\"), r"d:\a\b");
    }

    #[test]
    fn software_identity_requires_an_exact_process_name() {
        assert_eq!(application_name("Codex.exe"), Some("Codex"));
        assert_eq!(application_name("node.exe"), None);
        assert_eq!(application_name("my-codex-project.exe"), None);
    }

    #[test]
    fn metadata_refresh_reads_command_and_working_directory() {
        let mut sys = System::new();
        let pid = Pid::from_u32(std::process::id());
        refresh_pids(&mut sys, &HashSet::from([pid]));
        let own = sys.process(pid).expect("fixture process");
        assert!(!own.cmd().is_empty(), "default refresh omits command metadata");
        assert!(own.start_time() > 0, "ancestry must retain PID-reuse evidence");
        assert_eq!(own.cwd().unwrap().canonicalize().unwrap(), std::env::current_dir().unwrap().canonicalize().unwrap());
    }

    #[test]
    fn source_path_preserves_chinese_and_spaces() {
        assert_eq!(cwd_from_command(r#"node.exe "D:\项目\Lens Atlas\node_modules\vite\bin\vite.js""#), Some(r"D:\项目\Lens Atlas".into()));
        assert_eq!(cwd_from_command("node relative folder/node_modules/vite"), None);
    }
}
