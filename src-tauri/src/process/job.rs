//! Managed services join their Job before executing user code. Stopping a Job
//! requires no cmd/taskkill helper and includes descendants created afterwards.
#[cfg(windows)]
mod platform {
    use std::io;
    use std::mem::{size_of, zeroed};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::process::Child;
    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob,
        JobObjectBasicAccountingInformation, QueryInformationJobObject, TerminateJobObject,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, OpenThread, ResumeThread, PROCESS_QUERY_LIMITED_INFORMATION,
        THREAD_SUSPEND_RESUME,
    };

    pub struct ProcessJob(OwnedHandle);

    fn owned(raw: HANDLE) -> io::Result<OwnedHandle> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            // Each successful Win32 creation call transfers one unique handle.
            Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
        }
    }

    impl ProcessJob {
        pub fn new() -> io::Result<Self> {
            // Do not change the existing app-exit policy. Window close still
            // minimizes to tray; explicit Stop terminates this Job.
            owned(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) }).map(Self)
        }

        pub fn attach_and_resume(&self, child: &Child) -> io::Result<()> {
            let ok =
                unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), child.as_raw_handle()) };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            let snapshot = owned(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
            let mut entry: THREADENTRY32 = unsafe { zeroed() };
            entry.dwSize = size_of::<THREADENTRY32>() as u32;
            let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
            while found != 0 {
                if entry.th32OwnerProcessID == child.id() {
                    let thread =
                        owned(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) })?;
                    if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                        return Err(io::Error::last_os_error());
                    }
                    return Ok(());
                }
                found = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
            }
            Err(io::Error::other("无法恢复启动线程"))
        }

        pub fn active_count(&self) -> io::Result<u32> {
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
            let ok = unsafe {
                QueryInformationJobObject(
                    self.0.as_raw_handle(),
                    JobObjectBasicAccountingInformation,
                    &mut info as *mut _ as *mut _,
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(info.ActiveProcesses)
            }
        }

        pub fn contains_pid(&self, pid: u32) -> bool {
            let Ok(process) =
                owned(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) })
            else {
                return false;
            };
            let mut inside = 0;
            unsafe {
                IsProcessInJob(process.as_raw_handle(), self.0.as_raw_handle(), &mut inside) != 0
                    && inside != 0
            }
        }

        pub fn terminate(&self) -> io::Result<()> {
            if unsafe { TerminateJobObject(self.0.as_raw_handle(), 1) } == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::{io, process::Child};
    pub struct ProcessJob;
    impl ProcessJob {
        pub fn new() -> io::Result<Self> {
            Err(io::Error::other("仅支持 Windows"))
        }
        pub fn attach_and_resume(&self, _: &Child) -> io::Result<()> {
            Err(io::Error::other("仅支持 Windows"))
        }
        pub fn active_count(&self) -> io::Result<u32> {
            Ok(0)
        }
        pub fn contains_pid(&self, _: u32) -> bool {
            false
        }
        pub fn terminate(&self) -> io::Result<()> {
            Err(io::Error::other("仅支持 Windows"))
        }
    }
}
pub use platform::ProcessJob;
