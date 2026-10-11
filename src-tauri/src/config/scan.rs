//! Directory discovery is bounded and never follows child symlinks/junctions.
use std::{collections::{HashSet, VecDeque}, fs, path::{Component, Path, PathBuf}};

pub const MAX_DEPTH: usize = 3;
pub(super) const MAX_DIRECTORIES: usize = 128;
const MAX_ENTRIES: usize = 4096;

#[derive(Default)]
pub(super) struct Budget { entries: usize }
impl Budget {
    pub(super) fn take_entry(&mut self) -> bool {
        if self.entries >= MAX_ENTRIES { return false; }
        self.entries += 1;
        true
    }
}
pub(super) fn entry_limit_warning() -> String {
    format!("扫描达到 {MAX_ENTRIES} 个目录项上限；请选择更具体的项目目录")
}

fn excluded(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with('.') || ["node_modules", "vendor", "venv", "env", "target", "dist", "build", "out", "coverage", "release", "src-tauri", "assets", "static", "public", "test", "tests", "fixtures", "examples", "demo", "docs", "_archive", "_tasks"].contains(&name.as_str())
}

fn reparse(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else { return true; };
    if metadata.file_type().is_symlink() { return true; }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 { return true; }
    }
    false
}

pub(super) fn within_scope(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else { return false; };
    let mut current = root.to_path_buf();
    let mut depth = 0;
    for component in relative.components() {
        match component {
            Component::Normal(name) => {
                depth += 1;
                if depth > MAX_DEPTH || excluded(&name.to_string_lossy()) { return false; }
                current.push(name);
                if reparse(&current) { return false; }
            }
            Component::CurDir => {}
            _ => return false,
        }
    }
    match (root.canonicalize(), path.canonicalize()) {
        (Ok(root), Ok(path)) => path.starts_with(root) && path.is_dir(),
        _ => false,
    }
}

pub(super) fn discover(root: &Path, preferred: impl Iterator<Item=PathBuf>, budget: &mut Budget) -> (Vec<PathBuf>, Vec<String>) {
    let mut queue = VecDeque::from([root.to_path_buf()]);
    queue.extend(preferred.take(MAX_DIRECTORIES).filter(|path| within_scope(root, path)));
    let mut seen = HashSet::new();
    let mut directories = Vec::new();
    let mut warnings = Vec::new();
    while let Some(path) = queue.pop_front() {
        if !within_scope(root, &path) { continue; }
        let Ok(canonical) = path.canonicalize() else { continue; };
        // Windows paths are case insensitive; canonical identity also merges aliases.
        let key = canonical.to_string_lossy().to_lowercase();
        if !seen.insert(key) { continue; }
        if directories.len() >= MAX_DIRECTORIES {
            warnings.push(format!("扫描达到 {MAX_DIRECTORIES} 个目录上限；未发现的服务可选择更具体的项目目录或手动添加"));
            break;
        }
        let depth = path.strip_prefix(root).unwrap().components().filter(|part| matches!(part,Component::Normal(_))).count();
        directories.push(path.clone());
        if depth == MAX_DEPTH { continue; }
        let Ok(children) = fs::read_dir(&path) else {
            warnings.push(format!("无法读取目录 {}，已跳过",path.display()));
            continue;
        };
        let mut next = Vec::new();
        for child in children {
            if !budget.take_entry() {
                warnings.push(entry_limit_warning());
                return (directories,warnings);
            }
            let Ok(child) = child else { continue; };
            if excluded(&child.file_name().to_string_lossy()) { continue; }
            if child.file_type().is_ok_and(|kind| kind.is_dir()) && !reparse(&child.path()) {
                next.push(child.path());
            }
        }
        next.sort();
        queue.extend(next);
    }
    (directories,warnings)
}
