use crate::error::{err, Result};
use crate::services::paths::{ensure_launcher_dirs, launcher_root, profiles_file, settings_file};
use std::io::{Read, Write};
use std::path::Path;

pub fn export_bundle(dest: &Path) -> Result<String> {
    ensure_launcher_dirs()?;
    let file = std::fs::File::create(dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    add_file_to_zip(&mut zip, &profiles_file(), "profiles.json", options)?;
    if settings_file().exists() {
        add_file_to_zip(&mut zip, &settings_file(), "launcher.json", options)?;
    }

    zip.finish().map_err(|e| err(e.to_string()))?;
    Ok(dest.to_string_lossy().to_string())
}

fn add_file_to_zip(
    zip: &mut zip::ZipWriter<std::fs::File>,
    src: &Path,
    name: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<()> {
    zip.start_file(name, options)
        .map_err(|e| err(e.to_string()))?;
    let data = std::fs::read(src)?;
    zip.write_all(&data).map_err(|e| err(e.to_string()))?;
    Ok(())
}

pub fn import_bundle(src: &Path) -> Result<u32> {
    let file = std::fs::File::open(src)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| err(e.to_string()))?;
    ensure_launcher_dirs()?;

    let mut imported = 0u32;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| err(e.to_string()))?;
        let name = entry.name();
        if name == "profiles.json" {
            let mut data = String::new();
            entry.read_to_string(&mut data)?;
            let dest = profiles_file();
            if dest.exists() {
                merge_profiles(&data)?;
            } else {
                std::fs::write(dest, data)?;
            }
            imported += 1;
        } else if name == "launcher.json" {
            let mut data = String::new();
            entry.read_to_string(&mut data)?;
            std::fs::write(settings_file(), data)?;
            imported += 1;
        }
    }

    Ok(imported)
}

fn merge_profiles(imported_json: &str) -> Result<()> {
    let imported: serde_json::Value = serde_json::from_str(imported_json)?;
    let existing_data = std::fs::read_to_string(profiles_file())?;
    let existing: serde_json::Value = serde_json::from_str(&existing_data)?;

    let mut merged = existing["profiles"].as_array().cloned().unwrap_or_default();
    let import_arr = imported["profiles"].as_array().cloned().unwrap_or_default();

    for p in import_arr {
        let id = p["id"].as_str().unwrap_or("");
        if !crate::services::paths::is_safe_path_component(id) {
            continue;
        }
        if !merged.iter().any(|x| x["id"].as_str() == Some(id)) {
            merged.push(p);
        }
    }

    let out = serde_json::json!({ "profiles": merged });
    std::fs::write(profiles_file(), serde_json::to_string_pretty(&out)?)?;
    Ok(())
}

pub fn default_export_path() -> std::path::PathBuf {
    launcher_root().join("horizonui-launcher-export.zip")
}
