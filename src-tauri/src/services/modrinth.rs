use crate::error::{err, Result};
use crate::models::{
    ModBrowsePage, ModDownloadSelection, ModSearchResult, ModVersionFile, ModVersionInfo,
    ModrinthRelease,
};
use crate::services::{mods, versions};
use serde::Deserialize;

const MODRINTH_ID: &str = "90mpuiZs";

#[derive(Debug, Clone)]
pub struct ModrinthVersionResolved {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub filename: String,
    pub download_url: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ModrinthDependencyRef {
    pub project_id: String,
    pub version_id: Option<String>,
}

#[derive(Deserialize, Clone)]
struct ModrinthVersion {
    id: String,
    project_id: Option<String>,
    name: String,
    files: Vec<ModrinthFile>,
    dependencies: Option<Vec<ModrinthDependency>>,
}

#[derive(Deserialize, Clone)]
struct ModrinthDependency {
    project_id: Option<String>,
    version_id: Option<String>,
    dependency_type: String,
}

#[derive(Deserialize)]
struct ModrinthProject {
    title: String,
}

#[derive(Deserialize, Clone)]
struct ModrinthFile {
    filename: String,
    url: String,
    size: u64,
    primary: Option<bool>,
}

#[derive(Deserialize)]
struct ModrinthSearchHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    author: String,
    downloads: u64,
    icon_url: Option<String>,
}

#[derive(Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<ModrinthSearchHit>,
    offset: u32,
    limit: u32,
    total_hits: u32,
}

pub async fn fetch_release(mc_version: &str, loader: &str) -> Result<ModrinthRelease> {
    let mod_mc = versions::mod_mc_version(mc_version);
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version?loaders={}&game_versions=[\"{}\"]",
        MODRINTH_ID,
        modrinth_loader_query(loader),
        mod_mc
    );
    let client = reqwest::Client::new();
    let versions: Vec<ModrinthVersion> = client.get(&url).send().await?.json().await?;
    let latest = versions
        .first()
        .ok_or_else(|| err(format!("No HorizonUI release for {} {}", loader, mod_mc)))?;
    let file = pick_file(latest)?;
    Ok(ModrinthRelease {
        version: latest.name.clone(),
        version_id: latest.id.clone(),
        filename: file.filename.clone(),
        download_url: file.url.clone(),
    })
}

pub async fn fetch_version(version_id: &str) -> Result<ModrinthVersionResolved> {
    let client = reqwest::Client::new();
    let version: ModrinthVersion = client
        .get(format!(
            "https://api.modrinth.com/v2/version/{}",
            version_id
        ))
        .send()
        .await?
        .json()
        .await?;
    let file = pick_file(&version)?;
    let project_id = version
        .project_id
        .clone()
        .unwrap_or_else(|| version_id.to_string());
    Ok(ModrinthVersionResolved {
        id: version.id.clone(),
        project_id,
        name: version.name.clone(),
        filename: file.filename.clone(),
        download_url: file.url.clone(),
        size_bytes: file.size,
    })
}

pub async fn fetch_version_dependencies(version_id: &str) -> Result<Vec<ModrinthDependencyRef>> {
    let client = reqwest::Client::new();
    let version: ModrinthVersion = client
        .get(format!(
            "https://api.modrinth.com/v2/version/{}",
            version_id
        ))
        .send()
        .await?
        .json()
        .await?;

    Ok(version
        .dependencies
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.dependency_type == "required")
        .filter_map(|d| {
            let project_id = d.project_id?;
            Some(ModrinthDependencyRef {
                project_id,
                version_id: d.version_id,
            })
        })
        .collect())
}

pub async fn resolve_dependency(
    _profile_id: &str,
    project_id: &str,
    version_id: Option<&str>,
    loader: &str,
    mc_version: &str,
) -> Result<Option<ModDownloadSelection>> {
    let mod_mc = versions::mod_mc_version(mc_version);
    let client = reqwest::Client::new();

    let version = if let Some(id) = version_id {
        client
            .get(format!("https://api.modrinth.com/v2/version/{}", id))
            .send()
            .await?
            .json()
            .await?
    } else {
        let url = format!(
            "https://api.modrinth.com/v2/project/{}/version?loaders={}&game_versions=[\"{}\"]",
            project_id,
            modrinth_loader_query(loader),
            mod_mc
        );
        let versions: Vec<ModrinthVersion> = client.get(&url).send().await?.json().await?;
        versions.first().cloned().ok_or_else(|| {
            err(format!(
                "No compatible version for dependency {}",
                project_id
            ))
        })?
    };

    let file = pick_file(&version)?;
    let title = fetch_project_title(project_id)
        .await
        .unwrap_or_else(|_| project_id.to_string());

    Ok(Some(ModDownloadSelection {
        mod_id: project_id.to_string(),
        version_id: version.id.clone(),
        source: "modrinth".into(),
        title,
        version_name: version.name.clone(),
        filename: file.filename.clone(),
        download_url: file.url.clone(),
        size_bytes: file.size,
    }))
}

async fn fetch_project_title(project_id: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let project: ModrinthProject = client
        .get(format!(
            "https://api.modrinth.com/v2/project/{}",
            project_id
        ))
        .send()
        .await?
        .json()
        .await?;
    Ok(project.title)
}

fn modrinth_loader_query(loader: &str) -> String {
    match loader {
        "quilt" => r#"["quilt","fabric"]"#.into(),
        other => format!(r#"["{other}"]"#),
    }
}

fn modrinth_loader_facet(loader: &str) -> String {
    match loader {
        "quilt" => r#"["categories:quilt","categories:fabric"]"#.into(),
        other => format!(r#"["categories:{other}"]"#),
    }
}

fn pick_file(version: &ModrinthVersion) -> Result<&ModrinthFile> {
    version
        .files
        .iter()
        .find(|f| f.primary.unwrap_or(false) && !f.filename.contains("-cf"))
        .or_else(|| version.files.iter().find(|f| !f.filename.contains("-cf")))
        .or_else(|| version.files.iter().find(|f| f.primary.unwrap_or(false)))
        .or_else(|| version.files.first())
        .ok_or_else(|| err("No downloadable file on Modrinth"))
}

pub async fn download_to(path: &std::path::Path, url: &str) -> Result<()> {
    crate::services::download::assert_http_url(url)?;
    let client = reqwest::Client::new();
    let bytes = client.get(url).send().await?.bytes().await?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

fn modrinth_index(sort: &str) -> &'static str {
    match sort {
        "newest" => "newest",
        "updated" => "updated",
        "relevance" => "relevance",
        _ => "downloads",
    }
}

pub async fn browse(
    profile_id: &str,
    query: &str,
    loader: &str,
    mc_version: &str,
    offset: u32,
    limit: u32,
    sort: &str,
) -> Result<ModBrowsePage> {
    let mod_mc = versions::mod_mc_version(mc_version);
    let facets = format!(
        r#"[["project_type:mod"],["versions:{}"],{}]"#,
        mod_mc,
        modrinth_loader_facet(loader)
    );
    let index = if query.trim().is_empty() && sort == "relevance" {
        "downloads"
    } else {
        modrinth_index(sort)
    };

    let client = reqwest::Client::new();
    let resp: ModrinthSearchResponse = client
        .get("https://api.modrinth.com/v2/search")
        .query(&[
            ("query", query),
            ("limit", &limit.to_string()),
            ("offset", &offset.to_string()),
            ("index", index),
            ("facets", facets.as_str()),
        ])
        .send()
        .await?
        .json()
        .await?;

    let installed = mods::installed_filenames(profile_id);
    let results: Vec<ModSearchResult> = resp
        .hits
        .into_iter()
        .map(|h| {
            let slug = h.slug.clone();
            let slug_check = slug.clone();
            ModSearchResult {
                id: h.project_id,
                slug,
                title: h.title,
                description: h.description,
                icon_url: h.icon_url,
                author: h.author,
                downloads: h.downloads,
                source: "modrinth".into(),
                installed: installed.iter().any(|f| f.contains(&slug_check)),
            }
        })
        .collect();

    let next_offset = offset + results.len() as u32;
    let has_more = next_offset < resp.total_hits;

    Ok(ModBrowsePage {
        results,
        offset: next_offset,
        limit,
        total_hits: resp.total_hits,
        has_more,
    })
}

pub async fn get_versions(
    profile_id: &str,
    project_id: &str,
    loader: &str,
    mc_version: &str,
) -> Result<Vec<ModVersionInfo>> {
    let mod_mc = versions::mod_mc_version(mc_version);
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version?loaders={}&game_versions=[\"{}\"]",
        project_id,
        modrinth_loader_query(loader),
        mod_mc
    );
    let client = reqwest::Client::new();
    let versions: Vec<ModrinthVersion> = client.get(&url).send().await?.json().await?;
    let installed = mods::installed_filenames(profile_id);

    Ok(versions
        .into_iter()
        .map(|v| {
            let file = pick_file(&v).ok();
            let (filename, download_url, size_bytes) = if let Some(f) = file {
                (f.filename.clone(), f.url.clone(), f.size)
            } else {
                (String::new(), String::new(), 0)
            };
            let file_installed = mods::mark_installed(&installed, &filename);
            ModVersionInfo {
                id: v.id,
                name: v.name,
                files: if filename.is_empty() {
                    vec![]
                } else {
                    vec![ModVersionFile {
                        filename,
                        download_url,
                        size_bytes,
                    }]
                },
                installed: file_installed,
            }
        })
        .collect())
}
