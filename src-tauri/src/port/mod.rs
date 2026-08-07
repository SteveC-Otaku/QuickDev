use netstat2::{
    get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState,
};
use serde::Serialize;
use std::collections::HashMap;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Debug, Clone)]
pub struct RunningProject {
    pub id: String,
    pub name: String,
    pub root_pid: u32,
    pub port: Option<u16>,
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
}

pub fn list_listening_ports(
    highlight_ports: &[u16],
    owned_pids: &std::collections::HashSet<u32>,
    running_projects: &[RunningProject],
) -> Result<Vec<PortEntry>, String> {
    let sockets = get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP,
    )
    .map_err(|e| e.to_string())?;

    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let roots: HashMap<u32, (String, String)> = running_projects
        .iter()
        .map(|p| (p.root_pid, (p.id.clone(), p.name.clone())))
        .collect();

    let mut entries = Vec::new();

    for sock in sockets {
        let ProtocolSocketInfo::Tcp(tcp) = sock.protocol_socket_info else {
            continue;
        };
        if tcp.state != TcpState::Listen {
            continue;
        }

        let port = tcp.local_port;
        let highlight = highlight_ports.contains(&port);

        for pid in sock.associated_pids {
            let (process, command) = process_info(&sys, pid);
            let project = find_project_for_port(pid, port, &sys, &roots, running_projects);
            let owned = if project.is_some() || owned_pids.contains(&pid) {
                Some(true)
            } else {
                None
            };

            entries.push(PortEntry {
                port,
                process: project
                    .as_ref()
                    .map(|(_, name)| name.clone())
                    .unwrap_or(process),
                pid,
                command,
                highlight,
                owned,
                project_id: project.as_ref().map(|(id, _)| id.clone()),
                project_name: project.map(|(_, name)| name),
            });
        }
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

fn find_project_for_port(
    pid: u32,
    port: u16,
    sys: &System,
    roots: &HashMap<u32, (String, String)>,
    running_projects: &[RunningProject],
) -> Option<(String, String)> {
    if let Some(found) = find_project_by_ancestor(pid, sys, roots) {
        return Some(found);
    }

    for p in running_projects {
        if p.port == Some(port) {
            return Some((p.id.clone(), p.name.clone()));
        }
    }

    None
}

fn find_project_by_ancestor(
    pid: u32,
    sys: &System,
    roots: &HashMap<u32, (String, String)>,
) -> Option<(String, String)> {
    let mut current = Some(pid);
    let mut depth = 0;
    while let Some(p) = current {
        if let Some(proj) = roots.get(&p) {
            return Some(proj.clone());
        }
        current = sys
            .process(Pid::from_u32(p))
            .and_then(|proc| proc.parent())
            .map(|pp| pp.as_u32());
        depth += 1;
        if depth > 32 {
            break;
        }
    }
    None
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

    let command = if command.is_empty() { name.clone() } else { command };
    (name, command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_ports_smoke() {
        let ports = list_listening_ports(
            &[3000, 5173],
            &std::collections::HashSet::new(),
            &[],
        )
        .expect("scan");
        assert!(ports.iter().all(|p| p.port > 0));
    }
}
