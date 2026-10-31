use crate::error::Result;
use crate::services::paths::instance_dir;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceMeta {
    pub loader: String,
    pub version_id: String,
}

#[derive(Deserialize)]
struct LegacyMeta {
    fabric_version_id: Option<String>,
}

pub fn save_instance_meta(profile_id: &str, loader: &str, version_id: &str) -> Result<()> {
    let meta = InstanceMeta {
        loader: loader.into(),
        version_id: version_id.into(),
    };
    let path = instance_dir(profile_id).join("instance.json");
    std::fs::write(path, serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

pub fn load_instance_meta(profile_id: &str) -> Result<InstanceMeta> {
    let path = instance_dir(profile_id).join("instance.json");
    let data = std::fs::read_to_string(path)?;
    if let Ok(meta) = serde_json::from_str::<InstanceMeta>(&data) {
        return Ok(meta);
    }
    let legacy: LegacyMeta = serde_json::from_str(&data)?;
    Ok(InstanceMeta {
        loader: "fabric".into(),
        version_id: legacy.fabric_version_id.unwrap_or_else(|| "unknown".into()),
    })
}
