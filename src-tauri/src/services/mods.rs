use crate::error::{err, Result};
use crate::models::InstalledMod;
use crate::services::paths;

pub fn list_installed(profile_id: &str) -> Result<Vec<InstalledMod>> {
    let dir = paths::mods_dir(profile_id);
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut mods = Vec::new();
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "jar" || ext == "disabled" {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
                mods.push(InstalledMod {
                    filename,
                    size_bytes,
                });
            }
        }
    }
    mods.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(mods)
}

pub fn is_file_installed(profile_id: &str, filename: &str) -> bool {
    let Ok(name) = paths::safe_filename(filename) else {
        return false;
    };
    paths::mods_dir(profile_id).join(name).is_file()
}

pub async fn install_from_url(profile_id: &str, filename: &str, url: &str) -> Result<()> {
    paths::validate_path_component(profile_id)?;
    crate::services::download::assert_http_url(url)?;
    let dest = paths::mods_dir(profile_id).join(paths::safe_filename(filename)?);
    crate::services::modrinth::download_to(&dest, url).await?;
    Ok(())
}

pub fn remove(profile_id: &str, filename: &str) -> Result<()> {
    paths::validate_path_component(profile_id)?;
    let path = paths::mods_dir(profile_id).join(paths::safe_filename(filename)?);
    if !path.is_file() {
        return Err(err(format!("Mod not found: {}", filename)));
    }
    std::fs::remove_file(path)?;
    Ok(())
}

pub fn installed_filenames(profile_id: &str) -> Vec<String> {
    list_installed(profile_id)
        .map(|mods| mods.into_iter().map(|m| m.filename).collect())
        .unwrap_or_default()
}

pub fn mark_installed(installed: &[String], filename: &str) -> bool {
    installed.iter().any(|f| f == filename)
}
