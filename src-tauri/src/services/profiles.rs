use crate::error::{err, Result};
use crate::models::{LauncherSettings, Profile};
use crate::services::paths::{
    ensure_launcher_dirs, migrate_instance_layout, profiles_file, settings_file,
};
use chrono::Utc;
use uuid::Uuid;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ProfileStore {
    profiles: Vec<Profile>,
}

pub fn list() -> Result<Vec<Profile>> {
    ensure_launcher_dirs()?;
    let path = profiles_file();
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = std::fs::read_to_string(path)?;
    let store: ProfileStore = serde_json::from_str(&data)?;
    for profile in &store.profiles {
        if crate::services::paths::is_safe_path_component(&profile.id) {
            let _ = migrate_instance_layout(&profile.id);
        }
    }
    Ok(store.profiles)
}

fn save_all(profiles: &[Profile]) -> Result<()> {
    ensure_launcher_dirs()?;
    let store = ProfileStore {
        profiles: profiles.to_vec(),
    };
    std::fs::write(profiles_file(), serde_json::to_string_pretty(&store)?)?;
    Ok(())
}

pub fn create(
    name: String,
    mc_version: String,
    loader: String,
    ram_mb: u32,
    category: Option<String>,
) -> Result<Profile> {
    if !crate::services::versions::is_supported(&mc_version, &loader) {
        return Err(err(format!(
            "HorizonUI does not support {} on {}",
            loader, mc_version
        )));
    }
    let mut profiles = list()?;
    let profile = Profile {
        id: Uuid::new_v4().to_string(),
        name,
        mc_version,
        loader,
        ram_mb,
        java_path: None,
        created_at: Utc::now().to_rfc3339(),
        last_played: None,
        play_time_ms: 0,
        installed: false,
        category: category.unwrap_or_else(|| "General".into()),
    };
    std::fs::create_dir_all(crate::services::paths::game_dir(&profile.id))?;
    profiles.push(profile.clone());
    save_all(&profiles)?;
    Ok(profile)
}

pub fn update(profile: Profile) -> Result<Profile> {
    crate::services::paths::validate_path_component(&profile.id)?;
    let mut profiles = list()?;
    let idx = profiles
        .iter()
        .position(|p| p.id == profile.id)
        .ok_or_else(|| err("Profile not found"))?;
    profiles[idx] = profile.clone();
    save_all(&profiles)?;
    Ok(profile)
}

pub fn delete(id: &str) -> Result<()> {
    crate::services::paths::validate_path_component(id)?;
    let mut profiles = list()?;
    profiles.retain(|p| p.id != id);
    save_all(&profiles)?;
    let dir = crate::services::paths::instance_dir(id);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
    }
    Ok(())
}

pub fn get(id: &str) -> Result<Profile> {
    list()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| err("Profile not found"))
}

pub fn mark_installed(id: &str) -> Result<Profile> {
    let mut profile = get(id)?;
    profile.installed = true;
    update(profile)
}

pub fn add_play_time(id: &str, extra_ms: u64) -> Result<Profile> {
    if extra_ms == 0 {
        return get(id);
    }
    let mut profile = get(id)?;
    profile.play_time_ms = profile.play_time_ms.saturating_add(extra_ms);
    update(profile)
}

pub fn get_settings() -> Result<LauncherSettings> {
    ensure_launcher_dirs()?;
    let path = settings_file();
    if !path.exists() {
        return Ok(LauncherSettings::default());
    }
    let data = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

pub fn save_settings(settings: &LauncherSettings) -> Result<()> {
    ensure_launcher_dirs()?;
    std::fs::write(settings_file(), serde_json::to_string_pretty(settings)?)?;
    Ok(())
}
