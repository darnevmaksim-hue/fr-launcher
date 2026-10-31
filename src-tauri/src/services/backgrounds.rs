use crate::error::{err, Result};
use crate::models::BackgroundAsset;
use crate::services::paths::{game_dir, launcher_root};
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

const BUILTIN: &[&str] = &["default"];
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];
const NS_PREFIXES: &[&str] = &[
    "assets/horizonui/background/",
    "assets/horizonui/backgrounds/",
    "assets/nekoui/background/",
    "assets/nekoui/backgrounds/",
];

pub fn list_backgrounds(profile_id: &str) -> Result<Vec<BackgroundAsset>> {
    let mut assets: Vec<BackgroundAsset> = BUILTIN
        .iter()
        .map(|name| BackgroundAsset {
            name: (*name).into(),
            path: None,
            frames: Vec::new(),
            is_builtin: true,
        })
        .collect();
    let mut seen: HashSet<String> = BUILTIN.iter().map(|s| (*s).to_string()).collect();

    let packs = game_dir(profile_id).join("resourcepacks");
    if !packs.is_dir() {
        return Ok(assets);
    }

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&packs) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }

    for dir in dirs {
        scan_unpacked_pack(&dir, &mut assets, &mut seen);
    }
    for file in files {
        scan_zip_pack(profile_id, &file, &mut assets, &mut seen);
    }

    if assets.len() > 1 {
        assets[1..].sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }

    Ok(assets)
}

pub fn read_image_bytes(path: &str) -> Result<Vec<u8>> {
    let path = PathBuf::from(path);
    if !is_image_path(&path) {
        return Err(err("Not an image file"));
    }
    let root = launcher_root()
        .canonicalize()
        .unwrap_or_else(|_| launcher_root());
    let canonical = path
        .canonicalize()
        .map_err(|e| err(format!("Could not read image: {e}")))?;
    if !canonical.starts_with(&root) {
        return Err(err("Invalid image path"));
    }
    std::fs::read(canonical).map_err(|e| err(e.to_string()))
}

fn is_horizon_extension_dir(pack_dir: &Path) -> bool {
    pack_dir.join("assets/horizonui").is_dir() || pack_dir.join("assets/nekoui").is_dir()
}

fn scan_unpacked_pack(
    pack_dir: &Path,
    assets: &mut Vec<BackgroundAsset>,
    seen: &mut HashSet<String>,
) {
    if !is_horizon_extension_dir(pack_dir) {
        return;
    }

    let roots = [
        pack_dir.join("assets/horizonui/background"),
        pack_dir.join("assets/horizonui/backgrounds"),
        pack_dir.join("assets/nekoui/background"),
        pack_dir.join("assets/nekoui/backgrounds"),
    ];

    for root in roots {
        scan_background_root(&root, assets, seen);
    }
}

fn scan_background_root(
    root: &Path,
    assets: &mut Vec<BackgroundAsset>,
    seen: &mut HashSet<String>,
) {
    if !root.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = match path.file_name().and_then(|s| s.to_str()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => continue,
            };
            if seen.contains(&name) {
                continue;
            }
            let frames = images_in_dir(&path);
            if frames.is_empty() {
                continue;
            }
            seen.insert(name.clone());
            assets.push(BackgroundAsset {
                path: Some(frames[0].clone()),
                frames,
                name,
                is_builtin: false,
            });
        } else if path.is_file() {
            if !is_image_path(&path) {
                continue;
            }
            let name = match path.file_stem().and_then(|s| s.to_str()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => continue,
            };
            if seen.contains(&name) {
                continue;
            }
            let file = path.to_string_lossy().to_string();
            seen.insert(name.clone());
            assets.push(BackgroundAsset {
                name,
                path: Some(file.clone()),
                frames: vec![file],
                is_builtin: false,
            });
        }
    }
}

fn images_in_dir(dir: &Path) -> Vec<String> {
    let mut images: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_image_path(p))
        .collect();
    images.sort_by(|a, b| frame_sort_key(a).cmp(&frame_sort_key(b)));
    images
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect()
}

fn frame_sort_key(path: &Path) -> (u32, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    match stem.parse::<u32>() {
        Ok(n) => (n, stem.to_lowercase()),
        Err(_) => (u32::MAX, stem.to_lowercase()),
    }
}

fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| IMAGE_EXTS.iter().any(|ok| ext.eq_ignore_ascii_case(ok)))
        .unwrap_or(false)
}

fn is_image_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| IMAGE_EXTS.iter().any(|ok| ext.eq_ignore_ascii_case(ok)))
        .unwrap_or(false)
}

fn scan_zip_pack(
    profile_id: &str,
    zip_path: &Path,
    assets: &mut Vec<BackgroundAsset>,
    seen: &mut HashSet<String>,
) {
    let Ok(file) = File::open(zip_path) else {
        return;
    };
    let Ok(mut archive) = ZipArchive::new(file) else {
        return;
    };

    let mut is_extension = false;
    let mut grouped: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();

    for i in 0..archive.len() {
        let Ok(entry) = archive.by_index(i) else {
            continue;
        };
        let name = entry.name().replace('\\', "/");
        if name.contains("assets/horizonui/") || name.contains("assets/nekoui/") {
            is_extension = true;
        }
        if let Some((preset, entry_name)) = parse_background_zip_entry(&name) {
            grouped.entry(preset).or_default().push((i, entry_name));
        }
    }

    if !is_extension {
        return;
    }

    for (preset, mut files) in grouped {
        if seen.contains(&preset) {
            continue;
        }
        files.sort_by(|a, b| frame_sort_key(Path::new(&a.1)).cmp(&frame_sort_key(Path::new(&b.1))));
        let mut frames = Vec::new();
        for (index, entry_name) in files {
            if let Ok(preview) = extract_zip_preview(
                profile_id,
                zip_path,
                &mut archive,
                index,
                &entry_name,
                &preset,
            ) {
                frames.push(preview.to_string_lossy().to_string());
            }
        }
        if frames.is_empty() {
            continue;
        }
        seen.insert(preset.clone());
        assets.push(BackgroundAsset {
            path: Some(frames[0].clone()),
            frames,
            name: preset,
            is_builtin: false,
        });
    }
}

fn parse_background_zip_entry(name: &str) -> Option<(String, String)> {
    let n = name.trim_start_matches('/');
    for prefix in NS_PREFIXES {
        let Some(rest) = n.strip_prefix(prefix) else {
            continue;
        };
        if rest.is_empty() || rest.ends_with('/') {
            continue;
        }
        let mut parts = rest.split('/');
        let first = parts.next()?;
        match parts.next() {
            None => {
                if is_image_name(first) {
                    let preset = Path::new(first)
                        .file_stem()
                        .and_then(|s| s.to_str())?
                        .to_string();
                    return Some((preset, n.to_string()));
                }
            }
            Some(file) => {
                if is_image_name(file) && parts.next().is_none() {
                    return Some((first.to_string(), n.to_string()));
                }
            }
        }
    }
    None
}

fn extract_zip_preview(
    profile_id: &str,
    zip_path: &Path,
    archive: &mut ZipArchive<File>,
    index: usize,
    entry_name: &str,
    preset: &str,
) -> Result<PathBuf> {
    let file_name = Path::new(entry_name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("frame.png");
    let pack_stem = zip_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("pack");
    let cache_dir = launcher_root().join("background-cache").join(profile_id);
    std::fs::create_dir_all(&cache_dir)?;
    let dest = cache_dir.join(format!("{pack_stem}__{preset}__{file_name}"));
    if dest.is_file() {
        return Ok(dest);
    }

    let mut entry = archive.by_index(index).map_err(|e| err(e.to_string()))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    std::fs::write(&dest, buf)?;
    Ok(dest)
}

pub fn resolve_preview_path(profile_id: &str, background_name: &str) -> Result<Option<String>> {
    if background_name == "default" {
        return Ok(None);
    }
    let list = list_backgrounds(profile_id)?;
    Ok(list
        .into_iter()
        .find(|a| a.name == background_name)
        .and_then(|a| a.path))
}
