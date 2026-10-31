use crate::error::{err, Result};
use crate::services::paths::authlib_injector_jar;
use std::io::Read;
use std::path::Path;

const AUTHLIB_INJECTOR_FALLBACK_URL: &str =
    "https://github.com/yushijinhun/authlib-injector/releases/download/v1.2.8/authlib-injector-1.2.8.jar";
const MIN_JAR_BYTES: u64 = 10_000;

pub async fn ensure_jar() -> Result<std::path::PathBuf> {
    let path = authlib_injector_jar();
    if path.exists() && is_valid_jar(&path) {
        return Ok(path);
    }

    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let client = reqwest::Client::new();
    let url = resolve_download_url(&client)
        .await
        .unwrap_or_else(|_| AUTHLIB_INJECTOR_FALLBACK_URL.to_string());

    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        return Err(err(format!(
            "authlib-injector download failed (HTTP {}). URL: {}",
            response.status(),
            url
        )));
    }

    let bytes = response.bytes().await?;
    if bytes.len() < MIN_JAR_BYTES as usize || !bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        return Err(err(
            "authlib-injector download was invalid (not a JAR file). Check your internet connection.",
        ));
    }

    std::fs::write(&path, &bytes)?;
    Ok(path)
}

async fn resolve_download_url(client: &reqwest::Client) -> Result<String> {
    let response = client
        .get("https://api.github.com/repos/yushijinhun/authlib-injector/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "HorizonUI-Launcher")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(err(format!(
            "Failed to query authlib-injector release (HTTP {})",
            response.status()
        )));
    }

    let body: serde_json::Value = response.json().await?;
    let assets = body
        .get("assets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| err("authlib-injector release has no assets"))?;

    for asset in assets {
        let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.ends_with(".jar") {
            let url = asset
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| err("authlib-injector asset missing download URL"))?;
            return Ok(url.to_string());
        }
    }

    Err(err("No authlib-injector JAR found in latest release"))
}

fn is_valid_jar(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() < MIN_JAR_BYTES {
        return false;
    }

    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 4];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    header == [0x50, 0x4B, 0x03, 0x04]
}

pub fn jar_path_if_exists() -> Option<std::path::PathBuf> {
    let path = authlib_injector_jar();
    if path.exists() && is_valid_jar(&path) {
        Some(path)
    } else {
        None
    }
}

pub async fn ensure_jar_or_err() -> Result<std::path::PathBuf> {
    ensure_jar().await.map_err(|e| {
        err(format!(
            "Failed to download authlib-injector for Ely.by: {e}. Check your internet connection."
        ))
    })
}
