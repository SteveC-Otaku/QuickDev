use super::job::ProcessJob;

pub fn is_desktop_command(command: &str) -> bool {
    let words: Vec<_> = command
        .split_whitespace()
        .map(|part| part.trim_matches(['\'', '"']).to_ascii_lowercase())
        .collect();
    words
        .iter()
        .any(|part| matches!(part.as_str(), "electron" | "electron-vite"))
        || (words.iter().any(|part| part == "tauri") && words.iter().any(|part| part == "dev"))
}

#[cfg(windows)]
pub fn visible_window(job: &ProcessJob) -> Option<isize> {
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
        },
    };
    struct Search<'a> {
        job: &'a ProcessJob,
        found: Option<isize>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> i32 {
        let search = &mut *(data as *mut Search<'_>);
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if IsWindowVisible(hwnd) != 0
            && GetWindowTextLengthW(hwnd) > 0
            && search.job.contains_pid(pid)
        {
            search.found = Some(hwnd as isize);
            return 0;
        }
        1
    }
    let mut search = Search { job, found: None };
    unsafe {
        EnumWindows(Some(visit), &mut search as *mut _ as LPARAM);
    }
    search.found
}

#[cfg(windows)]
pub fn focus(job: &ProcessJob) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    let hwnd = visible_window(job).ok_or("App 窗口尚未出现，请查看编译日志")?
        as windows_sys::Win32::Foundation::HWND;
    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
        if SetForegroundWindow(hwnd) == 0 {
            return Err("Windows 未允许切换焦点，请从任务栏打开 App".into());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn visible_window(_: &ProcessJob) -> Option<isize> {
    None
}
#[cfg(not(windows))]
pub fn focus(_: &ProcessJob) -> Result<(), String> {
    Err("仅支持 Windows App 窗口".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn build_and_project_names_are_not_desktop_launches() {
        assert!(is_desktop_command("npm run tauri dev"));
        assert!(is_desktop_command("cargo tauri dev"));
        assert!(is_desktop_command("electron ."));
        assert!(!is_desktop_command("npm run tauri build"));
        assert!(!is_desktop_command("npm run dev --prefix tauri-site"));
    }
}
