use std::path::{Component, Path, PathBuf};

use crate::error::{err, Result};

/// Каталог данных лаунчера: инстансы, кэш, логи, настройки.
pub fn data_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fr-launcher")
}

/// Общие игровые данные (клиент, библиотеки, ассеты). Используем официальный
/// каталог .minecraft, чтобы не качать 7 ГБ заново и делить файлы с Electron-версией.
pub fn game_root() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join(".minecraft"))
        .unwrap_or_else(|| data_root().join("game"))
}

pub fn launcher_root() -> PathBuf {
    data_root()
}

pub fn profiles_file() -> PathBuf {
    data_root().join("profiles.json")
}

pub fn settings_file() -> PathBuf {
    data_root().join("launcher.json")
}

pub fn instances_dir() -> PathBuf {
    data_root().join("instances")
}

pub fn instance_dir(profile_id: &str) -> PathBuf {
    instances_dir().join(safe_component(profile_id))
}

/// Корень конкретного инстанса: здесь лежат mods/saves/config/options.txt.
pub fn game_dir(profile_id: &str) -> PathBuf {
    instance_dir(profile_id)
}

pub fn libraries_dir() -> PathBuf {
    game_root().join("libraries")
}

pub fn versions_dir() -> PathBuf {
    game_root().join("versions")
}

pub fn natives_dir(version_id: &str) -> PathBuf {
    data_root()
        .join("natives")
        .join(safe_component(version_id))
}

pub fn assets_dir() -> PathBuf {
    game_root().join("assets")
}

pub fn cache_dir() -> PathBuf {
    data_root().join("cache")
}

pub fn logs_dir() -> PathBuf {
    data_root().join("logs")
}

pub fn log_file() -> PathBuf {
    logs_dir().join("fr-launcher.log")
}

pub fn horizon_config_path(profile_id: &str) -> PathBuf {
    game_dir(profile_id).join("config").join("horizonui.json")
}

pub fn horizon_config_path_legacy(profile_id: &str) -> PathBuf {
    game_dir(profile_id)
        .join("config")
        .join("horizonui")
        .join("horizonui.json")
}

pub fn mods_dir(profile_id: &str) -> PathBuf {
    game_dir(profile_id).join("mods")
}

pub fn account_file() -> PathBuf {
    data_root().join("account.json")
}

pub fn accounts_file() -> PathBuf {
    data_root().join("accounts.json")
}

pub fn authlib_injector_jar() -> PathBuf {
    data_root().join("authlib-injector.jar")
}

pub fn ensure_launcher_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(instances_dir())?;
    std::fs::create_dir_all(libraries_dir())?;
    std::fs::create_dir_all(versions_dir())?;
    std::fs::create_dir_all(assets_dir())?;
    std::fs::create_dir_all(cache_dir())?;
    std::fs::create_dir_all(logs_dir())?;
    Ok(())
}

pub fn migrate_instance_layout(_profile_id: &str) -> std::io::Result<()> {
    // Раскладка FR-Launcher: инстанс = папка игры, общие libraries/versions/assets
    // лежат в .minecraft. Миграция из старой (HorizonUI) схемы не нужна.
    Ok(())
}

pub fn is_safe_path_component(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

fn safe_component(name: &str) -> &str {
    if is_safe_path_component(name) {
        name
    } else {
        "_"
    }
}

pub fn validate_path_component(name: &str) -> Result<()> {
    if is_safe_path_component(name) {
        Ok(())
    } else {
        Err(err("Invalid path"))
    }
}

pub fn safe_filename(name: &str) -> Result<String> {
    let base = Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| err("Invalid filename"))?;
    validate_path_component(base)?;
    Ok(base.to_string())
}

pub fn join_contained(base: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.replace('\\', "/");
    let rel_path = Path::new(&rel);
    if rel_path.is_absolute() {
        return Err(err("Invalid path"));
    }
    let mut out = base.to_path_buf();
    for component in rel_path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return Err(err("Invalid path")),
        }
    }
    Ok(out)
}
