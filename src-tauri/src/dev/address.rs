//! Log discoveries are session data, never persisted project overrides.
pub(super) fn port(url: &str) -> Option<u16> {
    url.parse::<tauri::Url>().ok()?.port_or_known_default()
}

pub(super) fn resolve(configured: Option<&str>, configured_port: Option<u16>, observed: Option<&str>) -> Option<String> {
    if let Some(url) = configured.filter(|url| !url.trim().is_empty()) { return Some(url.trim().into()); }
    if let Some(url) = observed.filter(|url| configured_port.is_none_or(|configured| port(url) == Some(configured))) {
        return Some(url.into());
    }
    configured_port.map(|port| format!("http://localhost:{port}"))
}

pub(super) fn parse_log_url(line: &str) -> Option<String> {
    // Drop CSI colour sequences before tokenizing the URL.
    let mut clean = String::new();
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for control in chars.by_ref() { if ('@'..='~').contains(&control) { break; } }
            }
        } else { clean.push(character); }
    }
    let lower = clean.to_ascii_lowercase();
    if !lower.contains("local:") { return None; }
    let start = lower.find("http://").or_else(|| lower.find("https://"))?;
    let raw = clean[start..].split_whitespace().next()?.trim_end_matches([')', ',', ';']);
    let url = raw.parse::<tauri::Url>().ok()?;
    if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty() || url.password().is_some() { return None; }
    let local = url.host_str().is_some_and(|host| host.eq_ignore_ascii_case("localhost")
        || host.trim_matches(['[', ']']).parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()));
    local.then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_paths_and_ports_win_without_mutating_configuration() {
        assert_eq!(resolve(Some("http://localhost:9001/admin?mode=dev"), Some(9001), Some("http://127.0.0.1:5174/")), Some("http://localhost:9001/admin?mode=dev".into()));
        assert_eq!(resolve(None, Some(9001), Some("http://127.0.0.1:5174/")), Some("http://localhost:9001".into()));
        assert_eq!(resolve(None, Some(5174), Some("http://127.0.0.1:5174/")), Some("http://127.0.0.1:5174/".into()));
        assert_eq!(resolve(None, None, Some("http://127.0.0.1:5174/")), Some("http://127.0.0.1:5174/".into()));
        assert_eq!(resolve(None, None, None), None);
    }
    #[test]
    fn parses_coloured_local_urls_and_rejects_remote_or_credential_urls() {
        assert_eq!(parse_log_url("➜ \u{1b}[32mLocal:\u{1b}[0m \u{1b}[36mhttp://127.0.0.1:5174/\u{1b}[0m"), Some("http://127.0.0.1:5174/".into()));
        assert_eq!(parse_log_url("Local: http://[::1]:5174/"), Some("http://[::1]:5174/".into()));
        assert!(parse_log_url("Local: https://example.com:5174/").is_none());
        assert!(parse_log_url("Local: http://user:password@localhost:5174/").is_none());
        assert!(parse_log_url("Network: http://localhost:5174/").is_none());
    }
}
