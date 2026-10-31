use crate::error::Result;
use crate::models::ChangelogFeed;
use serde::Deserialize;
use std::collections::HashMap;

const UPDATE_JSON_URL: &str =
    "https://raw.githubusercontent.com/nokarin-dev/horizonui/main/update/update.json";

pub async fn fetch_changelog() -> Result<ChangelogFeed> {
    let client = reqwest::Client::new();
    let json: serde_json::Value = client.get(UPDATE_JSON_URL).send().await?.json().await?;

    let homepage = json["homepage"]
        .as_str()
        .unwrap_or("https://modrinth.com/mod/horizonui")
        .to_string();

    let mut by_version: HashMap<String, crate::models::ChangelogEntry> = HashMap::new();
    if let Some(obj) = json.as_object() {
        for (mc_version, versions) in obj {
            if mc_version == "homepage" || mc_version == "promos" {
                continue;
            }
            if let Some(vers) = versions.as_object() {
                for (ver, text) in vers {
                    let Some(body) = text.as_str() else { continue };
                    let key = ver.trim().to_string();
                    by_version
                        .entry(key.clone())
                        .and_modify(|existing| {
                            if body.len() > existing.message.len() {
                                existing.message = body.to_string();
                                existing.mc_version = mc_version.clone();
                            }
                        })
                        .or_insert(crate::models::ChangelogEntry {
                            mc_version: mc_version.clone(),
                            version: key,
                            message: body.to_string(),
                        });
                }
            }
        }
    }

    let mut entries: Vec<_> = by_version.into_values().collect();
    entries.sort_by(|a, b| version_sort_key(&b.version).cmp(&version_sort_key(&a.version)));
    entries.truncate(12);

    Ok(ChangelogFeed { homepage, entries })
}

fn version_sort_key(ver: &str) -> (Vec<u32>, u8, u32) {
    let v = ver.trim().trim_start_matches('v');
    let (core, pre) = v.split_once('-').unwrap_or((v, ""));
    let core_parts: Vec<u32> = core.split('.').filter_map(|p| p.parse().ok()).collect();
    if pre.is_empty() {
        return (core_parts, 3, 0);
    }
    let (channel, num) = pre.split_once('.').unwrap_or((pre, "0"));
    let rank = match channel.to_ascii_lowercase().as_str() {
        "alpha" | "a" => 0,
        "beta" | "b" => 1,
        "rc" | "pre" => 2,
        _ => 1,
    };
    let n = num.parse().unwrap_or(0);
    (core_parts, rank, n)
}

#[derive(Deserialize)]
pub struct LauncherReleaseInfo {
    pub tag_name: String,
    pub html_url: String,
    pub body: Option<String>,
}

const LAUNCHER_GITHUB_API: &str =
    "https://api.github.com/repos/nokarin-dev/HorizonUI-Launcher/releases/latest";

pub async fn check_launcher_update(current: &str) -> Result<Option<LauncherReleaseInfo>> {
    let client = reqwest::Client::new();
    let res = client
        .get(LAUNCHER_GITHUB_API)
        .header("User-Agent", "HorizonUI-Launcher")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;

    if !res.status().is_success() {
        return Ok(None);
    }

    let release: LauncherReleaseInfo = res.json().await?;
    let latest = release.tag_name.trim_start_matches('v');
    let current = current.trim_start_matches('v');
    if latest != current && !latest.is_empty() {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}
