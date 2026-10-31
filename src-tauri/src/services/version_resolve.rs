use crate::error::{err, Result};
use crate::services::download::{
    is_native_library_name, library_allowed_os, library_artifact_path,
};
use crate::services::mojang;
use crate::services::paths::{cache_dir, libraries_dir, natives_dir, versions_dir};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct ResolvedLaunch {
    pub main_class: String,
    pub classpath: Vec<PathBuf>,
    pub version_id: String,
    pub jvm_args: Vec<String>,
}

pub fn resolve_launch(version_id: &str) -> Result<ResolvedLaunch> {
    let chain = load_version_chain(version_id)?;

    ensure_libraries(&chain)?;
    mojang::ensure_assets(&chain)?;

    let main_class = chain[0]["mainClass"]
        .as_str()
        .or_else(|| chain.last().and_then(|v| v["mainClass"].as_str()))
        .ok_or_else(|| err("mainClass missing"))?
        .to_string();

    let libs_dir = libraries_dir();
    let versions_root = versions_dir();
    let mut classpath = Vec::new();

    for json in chain.iter().rev() {
        collect_libraries(&libs_dir, json, &mut classpath);
        let id = json["id"].as_str().unwrap_or(version_id);
        let client = versions_root.join(id).join(format!("{id}.jar"));
        if client.exists() {
            classpath.push(client);
        }
    }

    let jvm_args = collect_jvm_args(&chain);
    ensure_natives(version_id)?;

    Ok(ResolvedLaunch {
        main_class,
        classpath,
        version_id: version_id.to_string(),
        jvm_args,
    })
}

fn load_version_chain(version_id: &str) -> Result<Vec<serde_json::Value>> {
    let mut chain = Vec::new();
    let mut current = version_id.to_string();
    let mut seen = HashSet::new();
    let versions_root = versions_dir();

    while !seen.contains(&current) {
        seen.insert(current.clone());
        let json_path = versions_root.join(&current).join(format!("{current}.json"));
        if !json_path.exists() {
            break;
        }
        let raw = std::fs::read_to_string(&json_path)?;
        let value: serde_json::Value = serde_json::from_str(&raw)?;
        current = value
            .get("inheritsFrom")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        chain.push(value);
        if current.is_empty() {
            break;
        }
    }

    if chain.is_empty() {
        return Err(err("Version json not found"));
    }
    Ok(chain)
}

fn collect_jvm_args(chain: &[serde_json::Value]) -> Vec<String> {
    let mut args = Vec::new();
    for json in chain.iter().rev() {
        if let Some(jvm) = json["arguments"]["jvm"].as_array() {
            push_version_arguments(jvm, &mut args);
        }
    }
    args
}

fn push_version_arguments(entries: &[serde_json::Value], out: &mut Vec<String>) {
    for entry in entries {
        if entry.is_string() {
            let value = entry.as_str().unwrap_or("").trim();
            if !value.is_empty() {
                out.push(value.to_string());
            }
            continue;
        }

        if !argument_rules_allow(entry.get("rules")) {
            continue;
        }

        match entry.get("value") {
            Some(serde_json::Value::String(value)) => {
                let value = value.trim();
                if !value.is_empty() {
                    out.push(value.to_string());
                }
            }
            Some(serde_json::Value::Array(values)) => {
                for value in values {
                    if let Some(text) = value.as_str() {
                        let text = text.trim();
                        if !text.is_empty() {
                            out.push(text.to_string());
                        }
                    }
                }
            }
            Some(_) | None => {}
        }
    }
}

fn argument_rules_allow(rules: Option<&serde_json::Value>) -> bool {
    let rules = match rules.and_then(|v| v.as_array()) {
        Some(rules) => rules,
        None => return true,
    };

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let mut allow = false;
    for rule in rules {
        let action = rule["action"].as_str().unwrap_or("allow");
        let os_name = rule["os"]["name"].as_str();
        let os_arch = rule["os"]["arch"].as_str();

        let os_match = match os_name {
            Some("linux") => os == "linux",
            Some("osx") => os == "macos",
            Some("windows") => os == "windows",
            None => true,
            _ => false,
        };

        let arch_match = match os_arch {
            Some(value) => arch == value,
            None => true,
        };

        if os_match && arch_match {
            allow = action == "allow";
        }
    }

    allow
}

fn collect_libraries(libs_dir: &Path, json: &serde_json::Value, classpath: &mut Vec<PathBuf>) {
    if let Some(libs) = json["libraries"].as_array() {
        for lib in libs {
            let rules = lib["rules"].as_array().map(|a| {
                serde_json::from_value::<Vec<crate::services::mojang::LibraryRule>>(
                    serde_json::Value::Array(a.clone()),
                )
                .unwrap_or_default()
            });
            if !library_allowed_os(rules.as_ref()) {
                continue;
            }

            let name = lib["name"].as_str().unwrap_or("");
            if name.is_empty() || is_native_library_name(name) {
                continue;
            }

            let artifact_path = lib["downloads"]["artifact"]["path"].as_str();
            let path = library_artifact_path(libs_dir, name, artifact_path);
            if path.exists() {
                classpath.push(path);
            }
        }
    }
}

pub fn ensure_libraries(chain: &[serde_json::Value]) -> Result<()> {
    let libs_dir = libraries_dir();

    for json in chain {
        if let Some(libs) = json["libraries"].as_array() {
            for lib in libs {
                let rules = lib["rules"].as_array().map(|a| {
                    serde_json::from_value::<Vec<crate::services::mojang::LibraryRule>>(
                        serde_json::Value::Array(a.clone()),
                    )
                    .unwrap_or_default()
                });
                if !library_allowed_os(rules.as_ref()) {
                    continue;
                }

                let name = lib["name"].as_str().unwrap_or("");
                if name.is_empty() {
                    continue;
                }

                let artifact_path = lib["downloads"]["artifact"]["path"].as_str();
                let dest = library_artifact_path(&libs_dir, name, artifact_path);
                if dest.exists() {
                    continue;
                }

                let url = if let Some(url) = lib["downloads"]["artifact"]["url"].as_str() {
                    url.to_string()
                } else if let Some(base_url) = lib["url"].as_str() {
                    fabric_library_url(base_url, name)?
                } else {
                    continue;
                };

                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let bytes = download_blocking(&url)?;
                std::fs::write(&dest, bytes)?;
            }
        }
    }

    Ok(())
}

fn fabric_library_url(base_url: &str, name: &str) -> Result<String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return Err(err(format!("Invalid library name: {name}")));
    }
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    Ok(format!(
        "{}/{}/{}/{}/{}-{}.jar",
        base_url.trim_end_matches('/'),
        group.replace('.', "/"),
        artifact,
        version,
        artifact,
        version
    ))
}

pub fn ensure_natives(version_id: &str) -> Result<PathBuf> {
    let natives_root = natives_dir(version_id);
    std::fs::create_dir_all(&natives_root)?;

    let chain = load_version_chain(version_id)?;
    for json in &chain {
        extract_natives_from_json(json, &natives_root)?;
    }

    Ok(natives_root)
}

fn extract_natives_from_json(json: &serde_json::Value, natives_dir: &Path) -> Result<()> {
    let libs_dir = libraries_dir();
    if let Some(libs) = json["libraries"].as_array() {
        for lib in libs {
            let rules = lib["rules"].as_array().map(|a| {
                serde_json::from_value::<Vec<crate::services::mojang::LibraryRule>>(
                    serde_json::Value::Array(a.clone()),
                )
                .unwrap_or_default()
            });
            if !library_allowed_os(rules.as_ref()) {
                continue;
            }

            let name = lib["name"].as_str().unwrap_or("");

            if is_native_library_name(name) {
                let artifact_path = lib["downloads"]["artifact"]["path"].as_str();
                let jar_path = library_artifact_path(&libs_dir, name, artifact_path);
                if !jar_path.exists() {
                    if let Some(url) = lib["downloads"]["artifact"]["url"].as_str() {
                        if let Some(parent) = jar_path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        let bytes = download_blocking(url)?;
                        std::fs::write(&jar_path, bytes)?;
                    }
                }
                if jar_path.exists() {
                    extract_zip_to(&jar_path, natives_dir)?;
                }
                continue;
            }

            if let Some(classifiers) = lib["downloads"]["classifiers"].as_object() {
                let native_key = if cfg!(windows) {
                    "natives-windows"
                } else if cfg!(target_os = "macos") {
                    "natives-macos"
                } else {
                    "natives-linux"
                };
                if let Some(native) = classifiers.get(native_key) {
                    if let Some(url) = native["url"].as_str() {
                        let bytes = download_blocking(url)?;
                        let tmp = cache_dir().join("native-tmp.zip");
                        std::fs::create_dir_all(cache_dir())?;
                        std::fs::write(&tmp, bytes)?;
                        extract_zip_to(&tmp, natives_dir)?;
                        let _ = std::fs::remove_file(&tmp);
                    }
                }
            }
        }
    }
    Ok(())
}

fn download_blocking(url: &str) -> Result<Vec<u8>> {
    crate::services::download::download_bytes_blocking(url)
}

fn extract_zip_to(zip_path: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    let dest = dest
        .canonicalize()
        .map_err(|e| err(format!("Invalid extract path: {e}")))?;
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| err(e.to_string()))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| err(e.to_string()))?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let out = dest.join(rel);
        if !out.starts_with(&dest) {
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out_file = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut out_file)?;
    }
    Ok(())
}
