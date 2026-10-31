use crate::error::{err, Result};
use crate::models::{
    ModBrowsePage, ModDownloadSelection, ModSearchResult, ModVersionFile, ModVersionInfo,
};
use crate::services::{mods, profiles, versions};
use reqwest::StatusCode;
use serde::Deserialize;

include!(concat!(env!("OUT_DIR"), "/curseforge_key.rs"));

const CF_GAME_ID: u32 = 432;
const CF_CLASS_MODS: u32 = 6;
const CF_API_BASES: &[&str] = &[
    "https://api.curseforge.com/v1",
    "https://api.curse.tools/v1/cf",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfSearchResponse {
    #[serde(default)]
    data: Vec<CfMod>,
    #[serde(default)]
    pagination: CfPagination,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CfPagination {
    #[serde(default)]
    total_count: u32,
    #[serde(default)]
    index: u32,
    #[serde(default)]
    page_size: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    summary: String,
    logo: Option<CfLogo>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
    #[serde(default)]
    download_count: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLogo {
    #[serde(default)]
    thumbnail_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfAuthor {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFilesResponse {
    #[serde(default)]
    data: Vec<CfFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u64,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    file_length: u64,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    is_available: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfDependenciesResponse {
    #[serde(default)]
    data: Vec<CfDependency>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfDependency {
    mod_id: u64,
    relation_type: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfModResponse {
    data: CfMod,
}

fn user_api_key() -> Option<String> {
    profiles::get_settings()
        .ok()
        .and_then(|s| s.curseforge_api_key)
        .map(|k| sanitize_api_key(&k))
        .filter(|k| !k.is_empty())
}

fn sanitize_api_key(raw: &str) -> String {
    raw.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

pub fn has_api_key() -> bool {
    user_api_key().is_some() || !sanitize_api_key(&embedded_curseforge_api_key()).is_empty()
}

fn builtin_api_key() -> Option<String> {
    let embedded = sanitize_api_key(&embedded_curseforge_api_key());
    if !embedded.is_empty() {
        return Some(embedded);
    }
    None
}

fn api_key_candidates() -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(user) = user_api_key() {
        keys.push(user);
    }
    if let Some(builtin) = builtin_api_key() {
        if !keys.iter().any(|k| k == &builtin) {
            keys.push(builtin);
        }
    }
    keys
}

fn cf_client() -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .user_agent("HorizonUI-Launcher/0.1")
        .http1_only()
        .timeout(std::time::Duration::from_secs(25));
    builder = builder.use_native_tls();
    builder.build().unwrap_or_else(|e| {
        log::warn!("CurseForge native-tls client failed ({e}); using default TLS");
        reqwest::Client::builder()
            .user_agent("HorizonUI-Launcher/0.1")
            .http1_only()
            .timeout(std::time::Duration::from_secs(25))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

fn key_meta(key: &str) -> String {
    format!("len={} prefix_ok={}", key.len(), key.starts_with("$2"))
}

fn body_preview(body: &str) -> String {
    body.chars()
        .take(400)
        .collect::<String>()
        .replace('\n', " ")
}

fn is_retryable_host_error(e: &crate::error::LauncherError) -> bool {
    let m = e.to_string();
    m.contains("refused the request")
        || m.contains("timed out the request")
        || m.contains("network filter")
        || m.contains("Could not reach CurseForge")
}

fn cf_error_message(status: StatusCode, body: &str) -> String {
    let lowered = body.to_ascii_lowercase();
    let looks_like_cloudflare =
        lowered.contains("cloudflare") || lowered.contains("cf-ray") || lowered.contains("<html");
    let json_message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("errorMessage")
                .or_else(|| value.get("message"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    if looks_like_cloudflare {
        return "CurseForge timed out the request (network filter). Try again in a moment.".into();
    }

    if let Some(message) = json_message {
        let msg_l = message.to_ascii_lowercase();
        if msg_l.contains("api key")
            || msg_l.contains("unauthorized")
            || msg_l.contains("forbidden")
        {
            return cf_forbidden_message();
        }
        return format!("CurseForge: {message}");
    }

    if status == StatusCode::UNAUTHORIZED {
        return cf_forbidden_message();
    }

    if status == StatusCode::FORBIDDEN {
        return "CurseForge refused the request. If this keeps happening, wait a bit and try again."
            .into();
    }

    format!("CurseForge request failed (HTTP {status}).")
}

fn cf_forbidden_message() -> String {
    if user_api_key().is_some() {
        "CurseForge rejected the API key. Clear the custom key in Settings → Data, or paste a key from console.curseforge.com.".into()
    } else {
        "CurseForge blocked the built-in key. Add your own API key in Settings → Data (console.curseforge.com).".into()
    }
}

fn loader_type(loader: &str) -> u32 {
    match loader {
        "forge" => 1,
        "fabric" => 4,
        "quilt" => 5,
        "neoforge" => 6,
        _ => 4,
    }
}

fn percent_encode_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for b in name.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn forgecdn_url(file_id: u64, file_name: &str) -> String {
    let a = file_id / 1000;
    let b = file_id % 1000;
    format!(
        "https://mediafilez.forgecdn.net/files/{a}/{b}/{}",
        percent_encode_filename(file_name)
    )
}

fn resolved_download_url(file: &CfFile) -> String {
    match file.download_url.as_deref() {
        Some(url) if !url.is_empty() => url.to_string(),
        _ => forgecdn_url(file.id, &file.file_name),
    }
}

fn loader_tags(loader: &str) -> &'static [&'static str] {
    match loader {
        "forge" => &["Forge"],
        "neoforge" => &["NeoForge"],
        "quilt" => &["Quilt", "Fabric"],
        _ => &["Fabric"],
    }
}

fn file_is_listed(file: &CfFile) -> bool {
    !file.file_name.is_empty() && !matches!(file.is_available, Some(false))
}

fn file_matches(file: &CfFile, mc_versions: &[String], loader: &str) -> bool {
    if !file_is_listed(file) {
        return false;
    }
    if file.game_versions.is_empty() {
        return true;
    }
    let has_mc = mc_versions
        .iter()
        .any(|mc| file.game_versions.iter().any(|v| v == mc));
    if !has_mc {
        return false;
    }
    let tags = loader_tags(loader);
    let mentions_loader = file.game_versions.iter().any(|v| {
        ["Forge", "Fabric", "NeoForge", "Quilt", "LiteLoader", "Rift"]
            .iter()
            .any(|known| known.eq_ignore_ascii_case(v))
    });
    if !mentions_loader {
        return true;
    }
    file.game_versions
        .iter()
        .any(|v| tags.iter().any(|tag| tag.eq_ignore_ascii_case(v)))
}

fn map_files(
    files: Vec<CfFile>,
    installed: &[String],
    mc_versions: &[String],
    loader: &str,
    require_match: bool,
) -> Vec<ModVersionInfo> {
    files
        .into_iter()
        .filter(|file| {
            if require_match {
                file_matches(file, mc_versions, loader)
            } else {
                file_is_listed(file)
            }
        })
        .map(|file| {
            let file_installed = mods::mark_installed(installed, &file.file_name);
            ModVersionInfo {
                id: file.id.to_string(),
                name: if file.display_name.is_empty() {
                    file.file_name.clone()
                } else {
                    file.display_name.clone()
                },
                files: vec![ModVersionFile {
                    filename: file.file_name.clone(),
                    download_url: resolved_download_url(&file),
                    size_bytes: file.file_length,
                }],
                installed: file_installed,
            }
        })
        .collect()
}

fn cf_sort_field(sort: &str) -> u32 {
    match sort {
        "newest" | "updated" => 3,
        "relevance" | "popularity" => 2,
        "name" => 4,
        _ => 6, // downloads
    }
}

async fn parse_cf_response<T: for<'de> Deserialize<'de>>(response: reqwest::Response) -> Result<T> {
    let status = response.status();
    let body = response.text().await.map_err(|e| {
        log::error!("CurseForge response body could not be read: {e}");
        err(format!("CurseForge response could not be read: {e}"))
    })?;

    log::debug!("CurseForge HTTP {status}: {}", body_preview(&body));

    if !status.is_success() {
        log::warn!(
            "CurseForge request failed ({status}): {}",
            body_preview(&body)
        );
        return Err(err(cf_error_message(status, &body)));
    }

    serde_json::from_str(&body)
        .map_err(|e| err(format!("CurseForge returned an unexpected response ({e}).")))
}

pub async fn browse(
    profile_id: &str,
    query: &str,
    offset: u32,
    limit: u32,
    sort: &str,
) -> Result<ModBrowsePage> {
    let keys = api_key_candidates();
    if keys.is_empty() {
        return Err(err(
            "CurseForge search needs an API key. Add one in Settings → Data.",
        ));
    }

    let profile = profiles::get(profile_id)?;
    let mod_mc = versions::mod_mc_version(&profile.mc_version);
    let client = cf_client();

    log::info!(
        "CurseForge search query={:?} mc={} loader={} keys={}",
        query,
        mod_mc,
        profile.loader,
        keys.len()
    );

    let mut last_err = err("CurseForge search failed.");
    for base in CF_API_BASES {
        for (index, key) in keys.iter().enumerate() {
            let url = format!("{base}/mods/search");
            log::info!("CurseForge GET {url} ({})", key_meta(key));
            let mut request = client
                .get(&url)
                .header("x-api-key", key)
                .header("Accept", "application/json")
                .query(&[
                    ("gameId", CF_GAME_ID.to_string()),
                    ("classId", CF_CLASS_MODS.to_string()),
                    ("gameVersion", mod_mc.clone()),
                    ("modLoaderType", loader_type(&profile.loader).to_string()),
                    ("sortField", cf_sort_field(sort).to_string()),
                    ("sortOrder", "desc".into()),
                    ("index", offset.to_string()),
                    ("pageSize", limit.to_string()),
                ]);
            if !query.trim().is_empty() {
                request = request.query(&[("searchFilter", query)]);
            }

            let response = match request.send().await {
                Ok(resp) => resp,
                Err(e) => {
                    log::warn!("CurseForge GET {url} failed to send: {e}");
                    last_err = err(format!("Could not reach CurseForge: {e}"));
                    if index + 1 < keys.len() {
                        continue;
                    }
                    break;
                }
            };

            match parse_cf_response::<CfSearchResponse>(response).await {
                Ok(resp) => {
                    let installed = mods::installed_filenames(profile_id);
                    let results: Vec<ModSearchResult> = resp
                        .data
                        .into_iter()
                        .map(|m| {
                            let slug = m.slug.clone();
                            let author = m
                                .authors
                                .first()
                                .map(|a| a.name.clone())
                                .unwrap_or_else(|| "Unknown".into());
                            let slug_check = slug.clone();
                            ModSearchResult {
                                id: m.id.to_string(),
                                slug,
                                title: m.name,
                                description: m.summary,
                                icon_url: m.logo.and_then(|l| {
                                    if l.thumbnail_url.is_empty() {
                                        None
                                    } else {
                                        Some(l.thumbnail_url)
                                    }
                                }),
                                author,
                                downloads: m.download_count,
                                source: "curseforge".into(),
                                installed: installed.iter().any(|f| f.contains(&slug_check)),
                            }
                        })
                        .collect();

                    let next_offset = offset + results.len() as u32;
                    let has_more = next_offset < resp.pagination.total_count;

                    log::info!(
                        "CurseForge search ok via {url} total={}",
                        resp.pagination.total_count
                    );
                    return Ok(ModBrowsePage {
                        results,
                        offset: next_offset,
                        limit,
                        total_hits: resp.pagination.total_count,
                        has_more,
                    });
                }
                Err(e) => {
                    last_err = e;
                    if is_retryable_host_error(&last_err) {
                        break;
                    }
                    if index + 1 < keys.len() {
                        continue;
                    }
                }
            }
        }
    }

    Err(last_err)
}

pub async fn get_versions(profile_id: &str, mod_id: &str) -> Result<Vec<ModVersionInfo>> {
    let keys = api_key_candidates();
    if keys.is_empty() {
        return Err(err(
            "CurseForge search needs an API key. Add one in Settings → Data.",
        ));
    }

    let profile = profiles::get(profile_id)?;
    let mc_versions = versions::compatible_mc_versions(&profile.mc_version);
    let primary_mc = versions::mod_mc_version(&profile.mc_version);
    let loader = profile.loader.clone();
    let client = cf_client();
    let installed = mods::installed_filenames(profile_id);

    let queries: Vec<Vec<(&str, String)>> = vec![
        vec![
            ("gameVersion", primary_mc.clone()),
            ("modLoaderType", loader_type(&loader).to_string()),
            ("pageSize", "50".into()),
        ],
        vec![
            ("gameVersion", profile.mc_version.clone()),
            ("modLoaderType", loader_type(&loader).to_string()),
            ("pageSize", "50".into()),
        ],
        vec![("pageSize", "50".into())],
    ];

    log::info!(
        "CurseForge files mod={} mc={} loader={}",
        mod_id,
        primary_mc,
        loader
    );

    let mut last_err = err("CurseForge versions could not be loaded.");
    let mut saw_ok = false;
    for base in CF_API_BASES {
        for (index, key) in keys.iter().enumerate() {
            let files_url = format!("{base}/mods/{}/files", mod_id);
            for query in queries.iter() {
                log::debug!("CurseForge GET {files_url} ({})", key_meta(key));
                let response = match client
                    .get(&files_url)
                    .header("x-api-key", key)
                    .header("Accept", "application/json")
                    .query(query)
                    .send()
                    .await
                {
                    Ok(resp) => resp,
                    Err(e) => {
                        log::warn!("CurseForge GET {files_url} failed to send: {e}");
                        last_err = err(format!("Could not reach CurseForge: {e}"));
                        continue;
                    }
                };

                match parse_cf_response::<CfFilesResponse>(response).await {
                    Ok(resp) => {
                        saw_ok = true;
                        let mapped = map_files(resp.data, &installed, &mc_versions, &loader, true);
                        if !mapped.is_empty() {
                            return Ok(mapped);
                        }
                    }
                    Err(e) => {
                        last_err = e;
                        if is_retryable_host_error(&last_err) {
                            break;
                        }
                        if index + 1 < keys.len() {
                            break;
                        }
                    }
                }
            }
        }
    }

    if saw_ok {
        return Ok(vec![]);
    }

    Err(last_err)
}

#[derive(Debug, Clone)]
pub struct CurseforgeDependencyRef {
    pub project_id: String,
}

pub async fn fetch_file_dependencies(
    mod_id: &str,
    file_id: &str,
) -> Result<Vec<CurseforgeDependencyRef>> {
    let keys = api_key_candidates();
    if keys.is_empty() {
        return Ok(vec![]);
    }

    let client = cf_client();
    let mut last_err = err("CurseForge dependency lookup failed.");

    for base in CF_API_BASES {
        for (index, key) in keys.iter().enumerate() {
            let url = format!("{base}/mods/{}/files/{}/dependencies", mod_id, file_id);
            log::debug!("CurseForge GET {url} ({})", key_meta(key));
            let response = match client
                .get(&url)
                .header("x-api-key", key)
                .header("Accept", "application/json")
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    last_err = err(format!("Could not reach CurseForge: {e}"));
                    continue;
                }
            };

            match parse_cf_response::<CfDependenciesResponse>(response).await {
                Ok(resp) => {
                    return Ok(resp
                        .data
                        .into_iter()
                        .filter(|d| d.relation_type == 3 || d.relation_type == 1)
                        .map(|d| CurseforgeDependencyRef {
                            project_id: d.mod_id.to_string(),
                        })
                        .collect());
                }
                Err(e) => {
                    last_err = e;
                    if is_retryable_host_error(&last_err) {
                        break;
                    }
                    if index + 1 < keys.len() {
                        continue;
                    }
                }
            }
        }
    }

    Err(last_err)
}

pub async fn resolve_dependency(
    profile_id: &str,
    dep_mod_id: &str,
    loader: &str,
    mc_version: &str,
) -> Result<Option<ModDownloadSelection>> {
    let profile = profiles::get(profile_id)?;
    let mod_mc = versions::mod_mc_version(mc_version);
    let keys = api_key_candidates();
    if keys.is_empty() {
        return Ok(None);
    }

    let client = cf_client();
    let mut last_err = err("CurseForge dependency resolution failed.");

    for base in CF_API_BASES {
        for (index, key) in keys.iter().enumerate() {
            let url = format!("{base}/mods/{}/files", dep_mod_id);
            log::debug!("CurseForge GET {url} ({})", key_meta(key));
            let response = match client
                .get(&url)
                .header("x-api-key", key)
                .header("Accept", "application/json")
                .query(&[
                    ("gameVersion", mod_mc.clone()),
                    ("modLoaderType", loader_type(&profile.loader).to_string()),
                    ("pageSize", "20".into()),
                ])
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    last_err = err(format!("Could not reach CurseForge: {e}"));
                    break;
                }
            };

            match parse_cf_response::<CfFilesResponse>(response).await {
                Ok(resp) => {
                    let mc_versions = versions::compatible_mc_versions(mc_version);
                    let file = resp
                        .data
                        .into_iter()
                        .find(|f| file_matches(f, &mc_versions, loader));
                    let Some(file) = file else {
                        last_err = err(format!(
                            "No compatible CurseForge file for mod {}",
                            dep_mod_id
                        ));
                        if index + 1 < keys.len() {
                            continue;
                        }
                        break;
                    };

                    let title = fetch_mod_name(dep_mod_id, key)
                        .await
                        .unwrap_or_else(|_| format!("Mod {}", dep_mod_id));
                    let download_url = resolved_download_url(&file);
                    let version_name = if file.display_name.is_empty() {
                        file.file_name.clone()
                    } else {
                        file.display_name.clone()
                    };

                    return Ok(Some(ModDownloadSelection {
                        mod_id: dep_mod_id.to_string(),
                        version_id: file.id.to_string(),
                        source: "curseforge".into(),
                        title,
                        version_name,
                        filename: file.file_name.clone(),
                        download_url,
                        size_bytes: file.file_length,
                    }));
                }
                Err(e) => {
                    last_err = e;
                    if is_retryable_host_error(&last_err) {
                        break;
                    }
                    if index + 1 < keys.len() {
                        continue;
                    }
                }
            }
        }
    }

    Err(last_err)
}

async fn fetch_mod_name(mod_id: &str, api_key: &str) -> Result<String> {
    let client = cf_client();
    let mut last_err = err("CurseForge mod lookup failed.");
    for base in CF_API_BASES {
        let url = format!("{base}/mods/{mod_id}");
        log::debug!("CurseForge GET {url} ({})", key_meta(api_key));
        let response = match client
            .get(&url)
            .header("x-api-key", api_key)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                last_err = err(format!("Could not reach CurseForge: {e}"));
                continue;
            }
        };
        match parse_cf_response::<CfModResponse>(response).await {
            Ok(resp) => return Ok(resp.data.name),
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}
