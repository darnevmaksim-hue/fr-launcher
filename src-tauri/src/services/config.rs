use crate::error::Result;
use crate::models::HorizonConfig;
use crate::services::paths::{horizon_config_path, horizon_config_path_legacy};
use std::fs;

pub fn read(profile_id: &str) -> Result<HorizonConfig> {
    let path = horizon_config_path(profile_id);
    if path.exists() {
        let data = fs::read_to_string(path)?;
        return Ok(serde_json::from_str(&data)?);
    }

    let legacy = horizon_config_path_legacy(profile_id);
    if legacy.exists() {
        let data = fs::read_to_string(&legacy)?;
        let config: HorizonConfig = serde_json::from_str(&data)?;
        write(profile_id, &config)?;
        return Ok(config);
    }

    Ok(HorizonConfig::default())
}

pub fn write(profile_id: &str, config: &HorizonConfig) -> Result<()> {
    let path = horizon_config_path(profile_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(config)?)?;

    let legacy = horizon_config_path_legacy(profile_id);
    if legacy.exists() {
        let legacy_parent = legacy.parent().map(|p| p.to_path_buf());
        let _ = fs::remove_file(&legacy);
        if let Some(parent) = legacy_parent {
            if parent.exists() && parent.read_dir().map(|d| d.count()).unwrap_or(0) == 0 {
                let _ = fs::remove_dir(parent);
            }
        }
    }

    Ok(())
}
