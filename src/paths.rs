use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use anyhow::{Result, anyhow, bail};

pub fn slash_relative(path: &Path, root: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        anyhow!(
            "path '{}' is outside scan root '{}'",
            path.display(),
            root.display()
        )
    })?;
    Ok(relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

pub fn normalize_config_path(value: &str) -> Result<String> {
    let normalized = value.replace('\\', "/");
    let path = Path::new(&normalized);
    if path.is_absolute() {
        bail!("config path must be repository-relative: {value}");
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        bail!("config path must not contain '..': {value}");
    }
    let trimmed = normalized.trim_start_matches("./").trim_end_matches('/');
    if trimmed.is_empty() {
        bail!("config path must not be empty");
    }
    Ok(trimmed.to_owned())
}

pub fn absolute_from_cwd(path: &Path, cwd: &Path) -> PathBuf {
    let combined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    lexical_normalize(&combined)
}

pub fn is_within(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

pub fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(value) => normalized.push(value),
        }
    }
    normalized
}

pub fn git_bytes_to_os(bytes: &[u8]) -> Result<OsString> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(OsString::from_vec(bytes.to_vec()))
    }

    #[cfg(windows)]
    {
        let value = std::str::from_utf8(bytes).map_err(|_| {
            anyhow!("Git returned a non-UTF-8 path, which is unsupported on Windows")
        })?;
        Ok(OsString::from(value))
    }

    #[cfg(not(any(unix, windows)))]
    {
        let value =
            std::str::from_utf8(bytes).map_err(|_| anyhow!("Git returned a non-UTF-8 path"))?;
        Ok(OsString::from(value))
    }
}
