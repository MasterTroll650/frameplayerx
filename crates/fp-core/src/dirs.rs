//! XDG directories for config, data and cache.

use std::path::PathBuf;

fn xdg(var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(fallback)
        }
    }
}

/// `$XDG_CONFIG_HOME/frameplayerx` (settings, sources, credentials).
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("frameplayerx")
}

/// `$XDG_DATA_HOME/frameplayerx` (library database).
pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("frameplayerx")
}

/// `$XDG_CACHE_HOME/frameplayerx` (thumbnails, previews).
pub fn cache_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache").join("frameplayerx")
}

/// Default folder scanned for scripts when none sit next to the video
/// (DeoVR's convention).
pub fn interactive_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("frameplayerx/Interactive")
}
