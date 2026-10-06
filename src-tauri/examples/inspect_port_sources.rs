//! Read-only verification of the same port-source code used by the application.
#[allow(dead_code)]
#[path = "../src/process/job.rs"]
pub mod job;
mod process {
    pub use crate::job;
}
#[allow(dead_code)]
#[path = "../src/port/mod.rs"]
mod port;
fn main() {
    let wanted: Vec<u16> = std::env::args()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    let entries = port::list_listening_ports(&[], &std::collections::HashSet::new(), &[])
        .expect("read listening ports");
    let visible: Vec<_> = entries.into_iter().filter(|p| wanted.contains(&p.port) || (wanted.is_empty() && p.source_app.is_some())).map(|p| serde_json::json!({
        "port":p.port,"pid":p.pid,"process":p.process,"executable":p.executable,"cwd":p.cwd,"sourceApp":p.source_app,"sourceEvidence":p.source_evidence,"parentChain":p.parent_chain
    })).collect();
    println!("{}", serde_json::to_string_pretty(&visible).unwrap());
}
