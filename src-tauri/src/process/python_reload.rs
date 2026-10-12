use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const SOURCE: &str = include_str!("python_reload/sitecustomize.py");
static PREPARE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn prepare(base: &Path) -> io::Result<PathBuf> {
    let _lock = PREPARE_LOCK.lock().map_err(|_| io::Error::other("Python 兼容缓存锁不可用"))?;
    let mut hash = DefaultHasher::new();
    SOURCE.hash(&mut hash);
    let directory = base.join(format!("python-reload-{:016x}", hash.finish()));
    std::fs::create_dir_all(&directory)?;
    let target = directory.join("sitecustomize.py");
    if std::fs::read(&target).ok().as_deref() == Some(SOURCE.as_bytes()) {
        return Ok(directory);
    }
    if target.exists() {
        return Err(io::Error::other("Python 热重载兼容文件已损坏，请移除对应 runtime 缓存后重试"));
    }
    let temporary = directory.join(format!("sitecustomize-{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
    file.write_all(SOURCE.as_bytes())?;
    file.sync_all()?;
    drop(file);
    if let Err(error) = std::fs::rename(&temporary, &target) {
        let _ = std::fs::remove_file(&temporary);
        if std::fs::read(&target).ok().as_deref() != Some(SOURCE.as_bytes()) {
            return Err(error);
        }
    }
    Ok(directory)
}

pub fn configure(command: &mut Command) -> io::Result<()> {
    let base = crate::config::store::data_dir().map_err(io::Error::other)?.join("runtime");
    let directory = prepare(&base)?;
    let mut paths = vec![directory];
    if let Some(existing) = std::env::var_os("PYTHONPATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    command.env("PYTHONPATH", std::env::join_paths(paths).map_err(io::Error::other)?);
    command.env("QUICKDEV_RELOAD_GUARD", "1");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_runtime_is_reused_without_overwriting_corrupted_code() {
        let base = std::env::temp_dir().join(format!("quickdev-reload-test-{}", std::process::id()));
        let directory = prepare(&base).unwrap();
        assert_eq!(directory, prepare(&base).unwrap());
        let target = directory.join("sitecustomize.py");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), SOURCE);
        std::fs::write(&target, "changed").unwrap();
        assert!(prepare(&base).is_err());
        assert!(base.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(base).unwrap();
    }
}
