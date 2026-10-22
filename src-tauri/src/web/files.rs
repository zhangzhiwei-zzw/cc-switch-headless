//! Private, isolated browser transfers. Backups are managed separately.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub static TRANSFER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub const MAX_TRANSFER_BYTES: u64 = 512 * 1024 * 1024;
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

pub fn allocate(kind: &str, name: &str) -> Result<PathBuf, String> {
    cleanup()?;
    if usage()? >= MAX_TRANSFER_BYTES {
        return Err("临时传输目录已达到 512 MiB，请清理 uploads/exports 后重试".into());
    }
    let name = Path::new(name)
        .file_name()
        .filter(|name| *name != "." && *name != "..")
        .unwrap_or_else(|| std::ffi::OsStr::new("transfer.bin"));
    let dir = crate::config::get_app_config_dir()
        .join(kind)
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    Ok(dir.join(name))
}

pub fn usage() -> Result<u64, String> {
    fn size(path: &Path) -> std::io::Result<u64> {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.is_symlink() {
            return Ok(0);
        }
        if metadata.is_file() {
            return Ok(metadata.len());
        }
        let mut total = 0;
        for entry in std::fs::read_dir(path)? {
            total += size(&entry?.path())?;
        }
        Ok(total)
    }
    let root = crate::config::get_app_config_dir();
    ["uploads", "exports"].iter().try_fold(0, |total, kind| {
        let path = root.join(kind);
        if !path.exists() {
            return Ok(total);
        }
        size(&path)
            .map(|bytes| total + bytes)
            .map_err(|error| error.to_string())
    })
}

pub fn cleanup() -> Result<(), String> {
    for kind in ["uploads", "exports"] {
        let dir = crate::config::get_app_config_dir().join(kind);
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(dir).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let metadata =
                std::fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
            let expired = metadata
                .modified()
                .ok()
                .and_then(|time| SystemTime::now().duration_since(time).ok())
                .is_some_and(|age| age > MAX_AGE);
            if expired {
                let result = if metadata.is_dir() && !metadata.is_symlink() {
                    std::fs::remove_dir_all(entry.path())
                } else {
                    std::fs::remove_file(entry.path())
                };
                if let Err(error) = result {
                    log::warn!("清理临时传输失败: {error}");
                }
            }
        }
    }
    Ok(())
}

/// Reject traversal and symlinks escaping the static root, including on Windows.
pub fn static_path(root: &Path, requested: &str) -> Result<PathBuf, ()> {
    if requested.contains('\\')
        || requested.contains(':')
        || Path::new(requested)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(());
    }
    let root = root.canonicalize().map_err(|_| ())?;
    let candidate = root.join(requested);
    if candidate.exists() {
        let canonical = candidate.canonicalize().map_err(|_| ())?;
        if !canonical.starts_with(&root) {
            return Err(());
        }
        return Ok(canonical);
    }
    Ok(candidate)
}
