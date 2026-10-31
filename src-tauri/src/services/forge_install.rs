use crate::error::{err, Result};
use crate::services::download::{download_to, DownloadOpts, ProgressCtx};
use crate::services::paths::{cache_dir, game_root};
use std::process::{Command, Stdio};

pub async fn get_recommended_forge_version(mc_version: &str) -> Result<String> {
    let promos: serde_json::Value = reqwest::get(
        "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
    )
    .await?
    .json()
    .await?;

    let key = format!("{mc_version}-recommended");
    let version = promos["promos"][&key]
        .as_str()
        .or_else(|| promos["promos"][format!("{mc_version}-latest")].as_str())
        .ok_or_else(|| err(format!("No Forge build for Minecraft {}", mc_version)))?;

    Ok(version.to_string())
}

pub async fn install_forge(
    mc_version: &str,
    java_path: &str,
    progress: &ProgressCtx,
) -> Result<String> {
    progress.emit(
        "forge",
        60,
        "Resolving Forge version…",
        None,
        None,
        None,
        None,
        None,
    );
    let forge_version = get_recommended_forge_version(mc_version).await?;

    let installer_name = format!("forge-{mc_version}-{forge_version}-installer.jar");
    let installer_url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc_version}-{forge_version}/{installer_name}"
    );

    std::fs::create_dir_all(cache_dir())?;
    let installer_jar = cache_dir().join(&installer_name);
    progress.emit(
        "forge",
        62,
        format!("Downloading Forge {forge_version}…"),
        None,
        None,
        None,
        None,
        None,
    );
    download_to(
        &installer_url,
        &installer_jar,
        Some(progress),
        DownloadOpts::new("forge", installer_name.clone())
            .with_span(62, 6)
            .with_label(format!("Downloading Forge {forge_version}")),
    )
    .await?;

    progress.log("Running Forge installer (may take a minute)…");
    progress.emit(
        "forge",
        70,
        "Running Forge installer…",
        Some(&installer_name),
        None,
        None,
        None,
        None,
    );
    let install_root = game_root();
    let status = Command::new(java_path)
        .arg("-jar")
        .arg(&installer_jar)
        .arg("--installClient")
        .arg(&install_root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| err(format!("Failed to run Forge installer: {e}")))?;

    if !status.success() {
        return Err(err("Forge installer failed"));
    }

    crate::services::mojang::find_loader_version_id(mc_version, "forge")
}

fn neoforge_prefix(mc_version: &str) -> String {
    let parts: Vec<&str> = mc_version.split('.').collect();
    if parts.len() >= 3 && parts[0] == "1" {
        format!("{}.{}", parts[1], parts[2])
    } else if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        mc_version.to_string()
    }
}

pub async fn get_latest_neoforge_version(mc_version: &str) -> Result<String> {
    let prefix = neoforge_prefix(mc_version);
    let url = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
    let data: serde_json::Value = reqwest::get(url).await?.json().await?;
    let versions = data["versions"]
        .as_array()
        .ok_or_else(|| err("NeoForge version list missing"))?;

    let mut best: Option<String> = None;
    for v in versions {
        let s = v.as_str().unwrap_or("");
        if !s.starts_with(&prefix) {
            continue;
        }
        if s.contains("beta") || s.contains("alpha") {
            continue;
        }
        if best.as_ref().map_or(true, |b| s > b.as_str()) {
            best = Some(s.to_string());
        }
    }

    best.ok_or_else(|| err(format!("No NeoForge build for Minecraft {}", mc_version)))
}

pub async fn install_neoforge(
    mc_version: &str,
    java_path: &str,
    progress: &ProgressCtx,
) -> Result<String> {
    progress.emit(
        "neoforge",
        60,
        "Resolving NeoForge version…",
        None,
        None,
        None,
        None,
        None,
    );
    let neoforge_version = get_latest_neoforge_version(mc_version).await?;

    let installer_name = format!("neoforge-{neoforge_version}-installer.jar");
    let installer_url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{neoforge_version}/{installer_name}"
    );

    std::fs::create_dir_all(cache_dir())?;
    let installer_jar = cache_dir().join(&installer_name);
    progress.emit(
        "neoforge",
        62,
        format!("Downloading NeoForge {neoforge_version}…"),
        None,
        None,
        None,
        None,
        None,
    );
    download_to(
        &installer_url,
        &installer_jar,
        Some(progress),
        DownloadOpts::new("neoforge", installer_name.clone())
            .with_span(62, 6)
            .with_label(format!("Downloading NeoForge {neoforge_version}")),
    )
    .await?;

    progress.log("Running NeoForge installer (may take a minute)…");
    progress.emit(
        "neoforge",
        70,
        "Running NeoForge installer…",
        Some(&installer_name),
        None,
        None,
        None,
        None,
    );
    let install_root = game_root();
    let status = Command::new(java_path)
        .arg("-jar")
        .arg(&installer_jar)
        .arg("--installClient")
        .arg(&install_root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| err(format!("Failed to run NeoForge installer: {e}")))?;

    if !status.success() {
        return Err(err("NeoForge installer failed"));
    }

    crate::services::mojang::find_loader_version_id(mc_version, "neoforge")
}
