use crate::error::{err, Result};
use crate::services::download::{download_to, library_artifact_path, DownloadOpts, ProgressCtx};
use crate::services::paths::{libraries_dir, versions_dir};

#[derive(serde::Deserialize)]
struct LoaderMeta {
    loader: LoaderVersion,
}

#[derive(serde::Deserialize)]
struct LoaderVersion {
    version: String,
}

enum IntermediaryLoader {
    Fabric,
    Quilt,
}

impl IntermediaryLoader {
    fn id(&self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
        }
    }

    fn version_prefix(&self) -> &'static str {
        match self {
            Self::Fabric => "fabric-loader",
            Self::Quilt => "quilt-loader",
        }
    }

    fn meta_url(&self, mc_version: &str) -> String {
        match self {
            Self::Fabric => {
                format!("https://meta.fabricmc.net/v2/versions/loader/{mc_version}")
            }
            Self::Quilt => {
                format!("https://meta.quiltmc.org/v3/versions/loader/{mc_version}")
            }
        }
    }

    fn profile_url(&self, mc_version: &str, loader_version: &str) -> String {
        match self {
            Self::Fabric => format!(
                "https://meta.fabricmc.net/v2/versions/loader/{mc_version}/{loader_version}/profile/json"
            ),
            Self::Quilt => format!(
                "https://meta.quiltmc.org/v3/versions/loader/{mc_version}/{loader_version}/profile/json"
            ),
        }
    }

    fn default_maven(&self) -> &'static str {
        match self {
            Self::Fabric => "https://maven.fabricmc.net/",
            Self::Quilt => "https://maven.quiltmc.org/repository/release/",
        }
    }
}

pub async fn install_fabric(
    profile: &crate::models::Profile,
    base_version_id: &str,
    progress: &ProgressCtx,
) -> Result<String> {
    install_intermediary(
        IntermediaryLoader::Fabric,
        profile,
        base_version_id,
        progress,
    )
    .await
}

pub async fn install_quilt(
    profile: &crate::models::Profile,
    base_version_id: &str,
    progress: &ProgressCtx,
) -> Result<String> {
    install_intermediary(
        IntermediaryLoader::Quilt,
        profile,
        base_version_id,
        progress,
    )
    .await
}

async fn install_intermediary(
    kind: IntermediaryLoader,
    profile: &crate::models::Profile,
    base_version_id: &str,
    progress: &ProgressCtx,
) -> Result<String> {
    let mc_version = &profile.mc_version;
    let stage = kind.id();
    let label = kind.label();

    progress.check_cancelled()?;
    progress.emit(
        stage,
        72,
        format!("Fetching {label} loader…"),
        None,
        None,
        None,
        None,
        None,
    );

    let loaders: Vec<LoaderMeta> = reqwest::get(&kind.meta_url(mc_version))
        .await?
        .json()
        .await?;
    let loader_version = loaders
        .first()
        .ok_or_else(|| err(format!("No {label} loader for Minecraft {mc_version}")))?
        .loader
        .version
        .clone();

    let version_id = format!("{}-{loader_version}-{mc_version}", kind.version_prefix());
    let profile_json: serde_json::Value =
        reqwest::get(&kind.profile_url(mc_version, &loader_version))
            .await?
            .json()
            .await?;

    let version_dir = versions_dir().join(&version_id);
    std::fs::create_dir_all(&version_dir)?;

    let mut merged = profile_json;
    if let Some(obj) = merged.as_object_mut() {
        obj.insert(
            "inheritsFrom".into(),
            serde_json::Value::String(base_version_id.into()),
        );
    }

    if let Some(libs) = merged["libraries"].as_array() {
        let libs_dir = libraries_dir();
        let mut pending: Vec<(String, String, std::path::PathBuf)> = Vec::new();

        for lib in libs {
            let name = lib["name"].as_str().unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let dest = library_artifact_path(&libs_dir, name, None);
            if dest.exists() {
                continue;
            }
            let base_url = lib["url"].as_str().unwrap_or_else(|| kind.default_maven());
            let parts: Vec<&str> = name.split(':').collect();
            if parts.len() < 3 {
                continue;
            }
            let (group, artifact, version) = (parts[0], parts[1], parts[2]);
            let path = format!(
                "{}/{}/{}/{}/{}-{}.jar",
                base_url.trim_end_matches('/'),
                group.replace('.', "/"),
                artifact,
                version,
                artifact,
                version
            );
            pending.push((name.to_string(), path, dest));
        }

        let total = pending.len() as u32;
        progress.log(format!("{total} {label} libraries to download"));

        for (i, (name, url, dest)) in pending.iter().enumerate() {
            progress.check_cancelled()?;
            let idx = i as u32 + 1;
            let pct = 72 + ((i * 8) / pending.len().max(1)) as u8;
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            download_to(
                url,
                dest,
                Some(progress),
                DownloadOpts::new(stage, name.clone())
                    .with_span(pct, 1)
                    .with_label(format!("{label} library {idx}/{total}"))
                    .with_item(idx, total),
            )
            .await?;
        }
    }

    std::fs::write(
        version_dir.join(format!("{version_id}.json")),
        serde_json::to_string_pretty(&merged)?,
    )?;

    progress.emit(
        stage,
        80,
        format!("{label} loader ready"),
        None,
        None,
        None,
        None,
        None,
    );

    Ok(version_id)
}
