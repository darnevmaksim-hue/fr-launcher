use crate::error::{err, Result};
use crate::models::Profile;
use crate::services::paths;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default = "default_ram")]
    pub ram: u32,
    #[serde(default)]
    pub java_path: String,
    #[serde(default)]
    pub created: i64,
    /// Установлена ли версия (клиент + загрузчик).
    #[serde(default)]
    pub installed: bool,
    /// id версии в versions/ (например 1.20.1-forge-47.4.28).
    #[serde(default)]
    pub version_id: String,
    #[serde(default)]
    pub last_played: Option<i64>,
    #[serde(default)]
    pub play_time_ms: u64,
    #[serde(default)]
    pub mods: Vec<Value>,
}

fn default_ram() -> u32 {
    4096
}

pub fn canon_loader(loader: &str) -> String {
    match loader.to_lowercase().as_str() {
        "fabric" => "Fabric".into(),
        "forge" => "Forge".into(),
        "neoforge" => "NeoForge".into(),
        "quilt" => "Quilt".into(),
        _ => "Vanilla".into(),
    }
}

pub fn loader_key(loader: &str) -> String {
    match loader.to_lowercase().as_str() {
        "fabric" => "fabric".into(),
        "forge" => "forge".into(),
        "neoforge" => "neoforge".into(),
        "quilt" => "quilt".into(),
        _ => "vanilla".into(),
    }
}

pub fn store_path() -> PathBuf {
    paths::data_root().join("instances.json")
}

pub fn read_store() -> Vec<Instance> {
    let path = store_path();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn write_store(list: &[Instance]) -> Result<()> {
    std::fs::create_dir_all(paths::data_root())?;
    std::fs::write(store_path(), serde_json::to_string_pretty(list)?)?;
    Ok(())
}

pub fn get(id: &str) -> Option<Instance> {
    read_store().into_iter().find(|i| i.id == id)
}

pub fn update<F: FnOnce(&mut Instance)>(id: &str, f: F) -> Result<Instance> {
    let mut list = read_store();
    let inst = list
        .iter_mut()
        .find(|i| i.id == id)
        .ok_or_else(|| err(format!("Instance {id} not found")))?;
    f(inst);
    let out = inst.clone();
    write_store(&list)?;
    Ok(out)
}

pub fn create(data: &Value) -> Result<Instance> {
    let name = data
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("New Instance")
        .to_string();
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let id = format!(
        "{}-{:x}",
        if slug.is_empty() { "instance" } else { &slug },
        stamp
    );

    let inst = Instance {
        id: id.clone(),
        name,
        mc_version: data
            .get("mcVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("1.20.1")
            .to_string(),
        loader: canon_loader(data.get("loader").and_then(|v| v.as_str()).unwrap_or("vanilla")),
        loader_version: data
            .get("loaderVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        ram: data.get("ram").and_then(|v| v.as_u64()).unwrap_or(4096) as u32,
        java_path: data
            .get("javaPath")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        created: stamp as i64,
        installed: false,
        version_id: String::new(),
        last_played: None,
        play_time_ms: 0,
        mods: Vec::new(),
    };

    let dir = paths::instance_dir(&id);
    std::fs::create_dir_all(dir.join("mods"))?;
    std::fs::create_dir_all(dir.join("config"))?;

    let mut list = read_store();
    list.push(inst.clone());
    write_store(&list)?;
    Ok(inst)
}

pub fn delete(id: &str) -> Result<()> {
    let mut list = read_store();
    list.retain(|i| i.id != id);
    write_store(&list)?;
    let _ = std::fs::remove_dir_all(paths::instance_dir(id));
    Ok(())
}

pub fn save_mods(id: &str, mods: Vec<Value>) -> Result<Value> {
    let inst = update(id, |i| i.mods = mods)?;
    Ok(serde_json::to_value(inst)?)
}

pub fn list() -> Vec<Value> {
    read_store()
        .into_iter()
        .map(|i| {
            let files = mod_files(&i.id);
            let mut v = serde_json::to_value(&i).unwrap_or_else(|_| json!({}));
            if let Some(obj) = v.as_object_mut() {
                obj.insert("loader".into(), json!(canon_loader(&i.loader)));
                obj.insert("modsCount".into(), json!(files.len()));
            }
            v
        })
        .collect()
}

fn slug_from_filename(file: &str) -> String {
    let stem = file.trim_end_matches(".jar");
    // Отрезаем хвост "-1.2.3+mc1.20.1" / "-2.0.0-beta".
    let chars: Vec<char> = stem.chars().collect();
    let mut cut = chars.len();
    for idx in 0..chars.len() {
        if chars[idx] == '-' && chars.get(idx + 1).map(|c| c.is_ascii_digit()).unwrap_or(false) {
            cut = idx;
            break;
        }
    }
    chars[..cut]
        .iter()
        .collect::<String>()
        .to_lowercase()
        .replace(' ', "-")
}

pub fn mod_files(id: &str) -> Vec<Value> {
    let dir = paths::mods_dir(id);
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jar") {
            continue;
        }
        let file = entry.file_name().to_string_lossy().to_string();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(json!({
            "file": file,
            "path": path.to_string_lossy(),
            "size": size,
            "slug": slug_from_filename(&file),
        }));
    }
    out
}

pub fn delete_mod_by_slug(id: &str, slug: &str) -> bool {
    let files = mod_files(id);
    if let Some(f) = files.iter().find(|v| v["slug"] == slug) {
        if let Some(p) = f["path"].as_str() {
            return std::fs::remove_file(p).is_ok();
        }
    }
    false
}

pub fn delete_mod_file(id: &str, file: &str) -> bool {
    let Ok(name) = paths::safe_filename(file) else {
        return false;
    };
    std::fs::remove_file(paths::mods_dir(id).join(name)).is_ok()
}

pub fn files_in(id: &str, kind: &str) -> Vec<Value> {
    let sub = match kind {
        "shaders" => "shaderpacks",
        "resources" => "resourcepacks",
        _ => "mods",
    };
    let dir = paths::instance_dir(id).join(sub);
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "jar" && ext != "zip" {
            continue;
        }
        out.push(json!({
            "file": entry.file_name().to_string_lossy(),
            "path": path.to_string_lossy(),
            "size": entry.metadata().map(|m| m.len()).unwrap_or(0),
        }));
    }
    out
}

pub fn delete_file_in(id: &str, kind: &str, file: &str) -> bool {
    let sub = match kind {
        "shaders" => "shaderpacks",
        "resources" => "resourcepacks",
        _ => "mods",
    };
    let Ok(name) = paths::safe_filename(file) else {
        return false;
    };
    std::fs::remove_file(paths::instance_dir(id).join(sub).join(name)).is_ok()
}

pub fn to_profile(inst: &Instance) -> Profile {
    let installed = inst.installed
        && paths::instance_dir(&inst.id)
            .join("instance.json")
            .exists();
    Profile {
        id: inst.id.clone(),
        name: inst.name.clone(),
        mc_version: inst.mc_version.clone(),
        loader: loader_key(&inst.loader),
        ram_mb: if inst.ram == 0 { 4096 } else { inst.ram },
        java_path: if inst.java_path.is_empty() {
            None
        } else {
            Some(inst.java_path.clone())
        },
        created_at: chrono::DateTime::from_timestamp_millis(inst.created)
            .unwrap_or_else(|| chrono::Utc::now())
            .to_rfc3339(),
        last_played: None,
        play_time_ms: inst.play_time_ms,
        installed,
        category: String::new(),
    }
}

pub fn mark_installed(id: &str, version_id: &str) -> Result<()> {
    update(id, |i| {
        i.installed = true;
        i.version_id = version_id.to_string();
    })?;
    Ok(())
}

/// Ищет инстанс по папке игры (для событий запуска).
pub fn find_by_dir(dir: &Path) -> Option<Instance> {
    read_store()
        .into_iter()
        .find(|i| paths::instance_dir(&i.id) == dir)
}

/// Ищет уже установленную версию клиента в общем каталоге .minecraft/versions.
/// Нужно, чтобы подхватывать установки, сделанные Electron-версией лаунчера,
/// и не качать клиент заново.
pub fn detect_installed_version(mc: &str, loader_key_value: &str) -> Option<String> {
    let dir = paths::versions_dir();
    let entries = std::fs::read_dir(&dir).ok()?;
    let mut found: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !entry.path().join(format!("{name}.json")).exists() {
            continue;
        }
        let low = name.to_lowercase();
        let mc_low = mc.to_lowercase();
        let matches = match loader_key_value {
            "vanilla" => low == mc_low,
            "fabric" => low.starts_with("fabric-loader-") && low.ends_with(&format!("-{mc_low}")),
            "quilt" => low.starts_with("quilt-loader-") && low.ends_with(&format!("-{mc_low}")),
            "forge" => low.starts_with(&format!("{mc_low}-forge-")),
            "neoforge" => {
                (low.starts_with(&format!("{mc_low}-neoforge-"))
                    || (low.ends_with(&format!("-{mc_low}"))
                        && !low.starts_with("fabric-loader-")
                        && !low.starts_with("quilt-loader-")))
                    && low.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
            }
            _ => false,
        };
        if matches {
            found.push(name);
        }
    }
    found.sort();
    found.pop()
}