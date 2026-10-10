//! 設定の保存先(JSON)とフォントのキャッシュ先。デスクトップ用。Android はアプリが保存先を渡す。

use crate::model::Settings;
use std::path::PathBuf;

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// 設定ファイルのあるディレクトリ(`app` はアプリ名)。
pub fn config_dir(app: &str) -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        env_path("APPDATA")
    } else if cfg!(target_os = "macos") {
        env_path("HOME").map(|h| h.join("Library").join("Application Support"))
    } else {
        env_path("XDG_CONFIG_HOME").or_else(|| env_path("HOME").map(|h| h.join(".config")))
    };
    base.unwrap_or_else(std::env::temp_dir).join(app)
}

/// フォントなどのキャッシュ先ディレクトリ。
pub fn cache_dir(app: &str) -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        env_path("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        env_path("HOME").map(|h| h.join("Library").join("Caches"))
    } else {
        env_path("XDG_CACHE_HOME").or_else(|| env_path("HOME").map(|h| h.join(".cache")))
    };
    base.unwrap_or_else(std::env::temp_dir).join(app)
}

/// 設定を読む(無い・壊れているときは既定値)。
pub fn load(app: &str) -> Settings {
    std::fs::read_to_string(config_dir(app).join("settings.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 設定を保存する。
pub fn save(app: &str, settings: &Settings) -> Result<(), String> {
    let dir = config_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("settings.json"), json).map_err(|e| e.to_string())
}
