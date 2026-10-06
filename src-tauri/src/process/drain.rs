//! Retaining services also requires retaining readers for their output pipes.
//! A hidden mode of this executable drains inherited pipes until writers exit;
//! it does not initialize Tauri, touch configuration, or create a console.
#[cfg(windows)]
mod platform {
    use std::os::windows::{
        io::{AsHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    };
    use std::{
        fs::File,
        io,
        process::{Child, Command, Stdio},
    };
    pub struct RetainedOutput {
        stdout: Option<OwnedHandle>,
        stderr: Option<OwnedHandle>,
    }
    impl RetainedOutput {
        pub fn capture(child: &Child) -> io::Result<Self> {
            Ok(Self {
                stdout: child
                    .stdout
                    .as_ref()
                    .map(|pipe| pipe.as_handle().try_clone_to_owned())
                    .transpose()?,
                stderr: child
                    .stderr
                    .as_ref()
                    .map(|pipe| pipe.as_handle().try_clone_to_owned())
                    .transpose()?,
            })
        }
        pub fn preserve(&self) -> io::Result<Child> {
            fn stream(handle: &Option<OwnedHandle>) -> io::Result<Stdio> {
                Ok(match handle {
                    Some(h) => Stdio::from(File::from(h.try_clone()?)),
                    None => Stdio::null(),
                })
            }
            Command::new(std::env::current_exe()?)
                .arg("--qd-drain-logs")
                .stdin(stream(&self.stdout)?)
                .stdout(stream(&self.stderr)?)
                .stderr(Stdio::null())
                .creation_flags(0x08000000)
                .spawn()
        }
    }
    pub fn drain() {
        use windows_sys::Win32::{
            Foundation::INVALID_HANDLE_VALUE,
            System::Console::{GetStdHandle, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
        };
        std::thread::scope(|scope| {
            for kind in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE] {
                let handle = unsafe { GetStdHandle(kind) };
                if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                    continue;
                }
                let mut file = unsafe { File::from_raw_handle(handle) };
                scope.spawn(move || {
                    let _ = io::copy(&mut file, &mut io::sink());
                });
            }
        });
    }
}
#[cfg(not(windows))]
mod platform {
    use std::{io, process::Child};
    pub struct RetainedOutput;
    impl RetainedOutput {
        pub fn capture(_: &Child) -> io::Result<Self> {
            Ok(Self)
        }
        pub fn preserve(&self) -> io::Result<Child> {
            Err(io::Error::other("仅支持 Windows"))
        }
    }
    pub fn drain() {}
}
pub use platform::{drain, RetainedOutput};
