// ponytail: taskkill /T /F covers kill tree on Windows; Job Object when dev runner lands
pub fn kill_process_tree(pid: u32) -> Result<(), String> {
    if pid == 0 {
        return Err("invalid pid".into());
    }

    #[cfg(windows)]
    {
        let output = std::process::Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            format!("taskkill failed for pid {pid}")
        } else {
            stderr.trim().to_string()
        });
    }

    #[cfg(not(windows))]
    {
        Err("kill is only supported on Windows".into())
    }
}
