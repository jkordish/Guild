use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PathResolutionError {
    message: String,
}

impl PathResolutionError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PathResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PathResolutionError {}

/// Resolve the default local Guild registry root as `~/.guild`.
///
/// # Errors
///
/// Returns an error if the current user's home directory cannot be resolved.
pub fn default_registry_root() -> Result<PathBuf, PathResolutionError> {
    Ok(home_dir()?.join(".guild"))
}

/// Resolve the default global Codex config file path as `~/.codex/config.toml`.
///
/// # Errors
///
/// Returns an error if the current user's home directory cannot be resolved.
pub fn global_codex_config_path() -> Result<PathBuf, PathResolutionError> {
    Ok(home_dir()?.join(".codex").join("config.toml"))
}

fn home_dir() -> Result<PathBuf, PathResolutionError> {
    home_dir_os().map(PathBuf::from).ok_or_else(|| {
        PathResolutionError::new(
            "could not resolve the current user's home directory for the default `~/.guild` root",
        )
    })
}

fn home_dir_os() -> Option<OsString> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .or({
            #[cfg(windows)]
            {
                std::env::var_os("USERPROFILE")
                    .filter(|value| !value.is_empty())
                    .or_else(|| {
                        let home_drive = std::env::var_os("HOMEDRIVE");
                        let home_path = std::env::var_os("HOMEPATH");
                        match (home_drive, home_path) {
                            (Some(drive), Some(path)) if !drive.is_empty() && !path.is_empty() => {
                                let mut combined = drive;
                                combined.push(path);
                                Some(combined)
                            }
                            _ => None,
                        }
                    })
            }
            #[cfg(not(windows))]
            {
                None
            }
        })
}
