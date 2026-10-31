use crate::error::{err, Result};
use crate::services::download::{
    download_bytes_blocking, download_to, library_allowed_os, library_artifact_path, DownloadOpts,
    ProgressCtx,
};
use crate::services::paths::{assets_dir, libraries_dir, versions_dir};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct VersionManifest {
    pub versions: Vec<VersionEntry>,
}

#[derive(Deserialize)]
pub struct VersionEntry {
    pub id: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct VersionJson {
    pub downloads: Downloads,
    #[serde(default)]
    pub libraries: Vec<Library>,
    pub asset_index: Option<AssetIndex>,
}

#[derive(Deserialize)]
pub struct Downloads {
    pub client: DownloadEntry,
}

#[derive(Deserialize)]
pub struct DownloadEntry {
    pub url: String,
    pub path: Option<String>,
}

#[derive(Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<LibraryRule>>,
}

#[derive(Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<DownloadEntry>,
    pub classifiers: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Deserialize)]
pub struct LibraryRule {
    pub action: String,
    pub os: Option<LibraryOsRule>,
}

#[derive(Deserialize)]
pub struct LibraryOsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

struct PendingDownload {
    name: String,
    url: String,
    dest: PathBuf,
}

pub async fn install_minecraft_base(mc_version: &str, progress: &ProgressCtx) -> Result<String> {
    progress.check_cancelled()?;
    progress.emit(
        "minecraft",
        8,
        "Fetching Minecraft version manifest…",
        None,
        None,
        None,
        None,
        None,
    );

    let manifest: VersionManifest =
        reqwest::get("https://launchermeta.mojang.com/mc/game/version_manifest_v2.json")
            .await?
            .json()
            .await?;

    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == mc_version)
        .ok_or_else(|| err(format!("Minecraft {} not found in manifest", mc_version)))?;

    let raw: serde_json::Value = reqwest::get(&entry.url).await?.json().await?;
    let version_json: VersionJson = serde_json::from_value(raw.clone())?;

    let versions_root = versions_dir();
    let version_dir = versions_root.join(mc_version);
    std::fs::create_dir_all(&version_dir)?;

    progress.emit(
        "minecraft",
        12,
        "Downloading Minecraft client…",
        None,
        None,
        None,
        None,
        None,
    );
    let client_jar = version_dir.join(format!("{mc_version}.jar"));
    download_to(
        &version_json.downloads.client.url,
        &client_jar,
        Some(progress),
        DownloadOpts::new("minecraft", format!("{mc_version}.jar"))
            .with_span(12, 8)
            .with_label("Downloading Minecraft client"),
    )
    .await?;

    std::fs::write(
        version_dir.join(format!("{mc_version}.json")),
        serde_json::to_string_pretty(&raw)?,
    )?;

    let libs_dir = libraries_dir();
    let mut pending: Vec<PendingDownload> = Vec::new();

    for lib in &version_json.libraries {
        if !library_allowed_os(lib.rules.as_ref()) {
            continue;
        }
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let dest = library_artifact_path(&libs_dir, &lib.name, artifact.path.as_deref());
            if !dest.exists() {
                pending.push(PendingDownload {
                    name: lib.name.clone(),
                    url: artifact.url.clone(),
                    dest,
                });
            }
        }
    }

    let lib_total = pending.len() as u32;
    progress.log(format!("{lib_total} libraries to download"));
    progress.emit(
        "libraries",
        20,
        format!("Downloading libraries (0/{lib_total})…"),
        None,
        Some(0),
        Some(lib_total),
        None,
        None,
    );

    for (i, item) in pending.iter().enumerate() {
        progress.check_cancelled()?;
        let idx = i as u32 + 1;
        let pct = 20 + ((i * 25) / pending.len().max(1)) as u8;
        let short_name = short_library_name(&item.name);

        download_to(
            &item.url,
            &item.dest,
            Some(progress),
            DownloadOpts::new("libraries", short_name.clone())
                .with_span(pct, 2)
                .with_label(format!("Library {idx}/{lib_total}"))
                .with_item(idx, lib_total),
        )
        .await?;

        progress.emit(
            "libraries",
            20 + ((idx as usize * 25) / pending.len().max(1)) as u8,
            format!("Libraries {idx}/{lib_total}"),
            Some(&short_name),
            Some(idx),
            Some(lib_total),
            None,
            None,
        );
    }

    if let Some(index) = &version_json.asset_index {
        progress.emit(
            "assets",
            48,
            "Downloading asset index…",
            None,
            None,
            None,
            None,
            None,
        );
        install_assets(index, progress).await?;
    }

    Ok(mc_version.to_string())
}

fn short_library_name(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() >= 2 {
        format!("{}:{}", parts[1], parts.get(2).unwrap_or(&""))
    } else {
        name.to_string()
    }
}

async fn install_assets(index: &AssetIndex, progress: &ProgressCtx) -> Result<()> {
    let game_assets = assets_dir();
    let indexes_dir = game_assets.join("indexes");
    std::fs::create_dir_all(&indexes_dir)?;

    let index_path = indexes_dir.join(format!("{}.json", index.id));
    download_to(
        &index.url,
        &index_path,
        Some(progress),
        DownloadOpts::new("assets", format!("{}.json", index.id))
            .with_span(48, 2)
            .with_label("Downloading asset index"),
    )
    .await?;

    let index_data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path)?)?;
    let objects = index_data["objects"]
        .as_object()
        .ok_or_else(|| err("Invalid asset index"))?;

    let objects_dir = game_assets.join("objects");
    let mut pending: Vec<(String, PathBuf)> = Vec::new();

    for (_key, obj) in objects {
        let hash = obj["hash"].as_str().unwrap_or("");
        if hash.is_empty() {
            continue;
        }
        let prefix = hash.chars().take(2).collect::<String>();
        let dest = objects_dir.join(&prefix).join(hash);
        if !dest.exists() {
            pending.push((hash.to_string(), dest));
        }
    }

    let total = pending.len() as u32;
    progress.log(format!("{total} assets to download"));
    progress.emit(
        "assets",
        50,
        format!("Downloading assets (0/{total})…"),
        None,
        Some(0),
        Some(total),
        None,
        None,
    );

    for (i, (hash, dest)) in pending.iter().enumerate() {
        progress.check_cancelled()?;
        let idx = i as u32 + 1;
        let prefix = hash.chars().take(2).collect::<String>();
        let url = format!("https://resources.download.minecraft.net/{prefix}/{hash}");
        let pct = 50 + ((i * 20) / pending.len().max(1)) as u8;

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        download_to(
            &url,
            dest,
            Some(progress),
            DownloadOpts::new("assets", hash.clone())
                .with_span(pct, 1)
                .with_label(format!("Asset {idx}/{total}"))
                .with_item(idx, total),
        )
        .await?;
    }

    progress.emit(
        "assets",
        70,
        format!("Assets complete ({total} downloaded)"),
        None,
        Some(total),
        Some(total),
        None,
        None,
    );

    Ok(())
}

pub fn ensure_assets(chain: &[serde_json::Value]) -> Result<()> {
    let index = chain.iter().find_map(|v| {
        let ai = v.get("assetIndex")?;
        Some(AssetIndex {
            id: ai["id"].as_str()?.to_string(),
            url: ai["url"].as_str()?.to_string(),
        })
    });

    let index = match index {
        Some(i) => i,
        None => return Ok(()),
    };

    let game_assets = assets_dir();
    let indexes_dir = game_assets.join("indexes");
    std::fs::create_dir_all(&indexes_dir)?;

    let index_path = indexes_dir.join(format!("{}.json", index.id));
    if !index_path.exists() {
        let bytes = download_bytes_blocking(&index.url)?;
        std::fs::write(&index_path, &bytes)?;
    }

    let index_data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path)?)?;
    let objects = index_data["objects"]
        .as_object()
        .ok_or_else(|| err("Invalid asset index"))?;

    let objects_dir = game_assets.join("objects");
    let mut pending: Vec<(String, PathBuf)> = Vec::new();

    for (_key, obj) in objects {
        let hash = obj["hash"].as_str().unwrap_or("");
        if hash.is_empty() {
            continue;
        }
        let prefix = hash.chars().take(2).collect::<String>();
        let dest = objects_dir.join(&prefix).join(hash);
        if !dest.exists() {
            pending.push((hash.to_string(), dest));
        }
    }

    for (hash, dest) in pending {
        let prefix = hash.chars().take(2).collect::<String>();
        let url = format!("https://resources.download.minecraft.net/{prefix}/{hash}");
        let bytes = download_bytes_blocking(&url)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, &bytes)?;
    }

    Ok(())
}

pub fn find_loader_version_id(mc_version: &str, loader: &str) -> Result<String> {
    let versions = versions_dir();
    if !versions.exists() {
        return Err(err("No versions directory"));
    }
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(&versions)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        match loader {
            "fabric" if name.starts_with("fabric-loader-") && name.ends_with(mc_version) => {
                candidates.push(name);
            }
            "quilt" if name.starts_with("quilt-loader-") && name.ends_with(mc_version) => {
                candidates.push(name);
            }
            "forge" if name.contains("forge") && name.contains(mc_version) => {
                candidates.push(name);
            }
            "neoforge" if name.contains("neoforge") => {
                candidates.push(name);
            }
            _ => {}
        }
    }
    candidates
        .into_iter()
        .max_by_key(|s| s.len())
        .ok_or_else(|| err(format!("No {} version found for {}", loader, mc_version)))
}
