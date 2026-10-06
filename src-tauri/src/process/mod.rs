pub mod job;
pub mod drain;
pub mod window;

// Only external, explicitly confirmed processes use the taskkill fallback.
pub fn kill_process_tree(pid: u32) -> Result<(), String> {
    if pid <= 4 || pid == std::process::id() {
        return Err("不能结束系统进程或即栈自身".into());
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut child = std::process::Command::new("taskkill")
            .creation_flags(0x0800_0000)
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20))
                }
                outcome => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(match outcome {
                        Err(e) => e.to_string(),
                        _ => "结束进程超时，请刷新后确认进程状态".into(),
                    });
                }
            }
        }
        let output = child.wait_with_output().map_err(|e| e.to_string())?;

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
