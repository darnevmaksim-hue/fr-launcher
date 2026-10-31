use crate::error::{err, Result};
use crate::models::{InstallLogEntry, InstallProgress};
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

static INSTALL_CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn cancel_install() {
    INSTALL_CANCELLED.store(true, Ordering::SeqCst);
}

pub fn reset_install_cancel() {
    INSTALL_CANCELLED.store(false, Ordering::SeqCst);
}

pub fn is_install_cancelled() -> bool {
    INSTALL_CANCELLED.load(Ordering::SeqCst)
}

pub struct ProgressCtx {
    app: AppHandle,
}

impl ProgressCtx {
    pub fn new(app: AppHandle) -> Self {
        reset_install_cancel();
        Self { app }
    }

    pub fn check_cancelled(&self) -> Result<()> {
        if is_install_cancelled() {
            Err(err("Install cancelled"))
        } else {
            Ok(())
        }
    }

    pub fn log(&self, message: impl Into<String>) {
        let message = message.into();
        log::info!("install: {message}");
        let _ = self.app.emit(
            "install-log",
            InstallLogEntry {
                message: message.clone(),
                level: "info".into(),
            },
        );
        let _ = self.app.emit(
            "game:log",
            serde_json::json!({ "text": format!("{message}\n"), "cls": "ok" }),
        );
    }

    pub fn emit(
        &self,
        stage: &str,
        percent: u8,
        message: impl Into<String>,
        current_file: Option<&str>,
        item_index: Option<u32>,
        item_total: Option<u32>,
        bytes_downloaded: Option<u64>,
        bytes_total: Option<u64>,
    ) {
        let message = message.into();
        let file = current_file.map(|s| s.to_string());
        let _ = self.app.emit(
            "install-progress",
            InstallProgress {
                stage: stage.into(),
                percent,
                message: message.clone(),
                current_file: file.clone(),
                item_index,
                item_total,
                bytes_downloaded,
                bytes_total,
            },
        );
        // Формат прогресса нашего интерфейса (Electron: канал "prog").
        let (done, total) = match (bytes_downloaded, bytes_total) {
            (Some(d), Some(t)) if t > 0 => (d, t),
            _ => (percent as u64, 100u64),
        };
        let _ = self.app.emit(
            "prog",
            serde_json::json!({
                "done": done,
                "total": total,
                "label": message,
                "detail": file,
            }),
        );
    }

    pub fn emit_download(
        &self,
        stage: &str,
        percent_base: u8,
        percent_span: u8,
        message: &str,
        file_name: &str,
        item_index: Option<u32>,
        item_total: Option<u32>,
        bytes_downloaded: u64,
        bytes_total: Option<u64>,
    ) {
        let percent = if let Some(total) = bytes_total {
            if total > 0 {
                let span = (bytes_downloaded as u32 * percent_span as u32) / total as u32;
                percent_base
                    .saturating_add(span as u8)
                    .min(percent_base + percent_span)
            } else {
                percent_base
            }
        } else {
            percent_base
        };

        self.emit(
            stage,
            percent,
            message,
            Some(file_name),
            item_index,
            item_total,
            Some(bytes_downloaded),
            bytes_total,
        );
    }
}

pub struct DownloadOpts<'a> {
    pub stage: &'a str,
    pub percent_base: u8,
    pub percent_span: u8,
    pub label: String,
    pub file_name: String,
    pub item_index: Option<u32>,
    pub item_total: Option<u32>,
    pub skip_existing: bool,
}

impl<'a> DownloadOpts<'a> {
    pub fn new(stage: &'a str, file_name: impl Into<String>) -> Self {
        Self {
            stage,
            percent_base: 0,
            percent_span: 100,
            label: String::new(),
            file_name: file_name.into(),
            item_index: None,
            item_total: None,
            skip_existing: true,
        }
    }

    pub fn with_span(mut self, base: u8, span: u8) -> Self {
        self.percent_base = base;
        self.percent_span = span;
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_item(mut self, index: u32, total: u32) -> Self {
        self.item_index = Some(index);
        self.item_total = Some(total);
        self
    }

    pub fn skip_existing(mut self, skip: bool) -> Self {
        self.skip_existing = skip;
        self
    }
}

pub fn assert_http_url(url: &str) -> Result<()> {
    let url = url.trim();
    if url.len() > 2048 {
        return Err(err("Invalid URL"));
    }
    let ok = (url.starts_with("https://") || url.starts_with("http://"))
        && !url.contains(char::is_whitespace)
        && !url.contains('\0');
    if ok {
        Ok(())
    } else {
        Err(err("URL must be http or https"))
    }
}

pub async fn download_bytes(url: &str) -> Result<Vec<u8>> {
    assert_http_url(url)?;
    let client = reqwest::Client::new();
    let bytes = client.get(url).send().await?.bytes().await?;
    Ok(bytes.to_vec())
}

pub fn download_bytes_blocking(url: &str) -> Result<Vec<u8>> {
    assert_http_url(url)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| err(e.to_string()))?;
    let bytes = client
        .get(url)
        .send()
        .map_err(|e| err(e.to_string()))?
        .bytes()
        .map_err(|e| err(e.to_string()))?;
    Ok(bytes.to_vec())
}

pub async fn download_to(
    url: &str,
    dest: &Path,
    progress: Option<&ProgressCtx>,
    opts: DownloadOpts<'_>,
) -> Result<()> {
    assert_http_url(url)?;
    if let Some(p) = progress {
        p.check_cancelled()?;
    }

    if opts.skip_existing && dest.exists() {
        return Ok(());
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let display_label = if opts.label.is_empty() {
        format!("Downloading {}", opts.file_name)
    } else {
        opts.label.clone()
    };

    if let Some(p) = progress {
        p.log(format!("Downloading: {}", opts.file_name));
        p.emit(
            opts.stage,
            opts.percent_base,
            &display_label,
            Some(&opts.file_name),
            opts.item_index,
            opts.item_total,
            Some(0),
            None,
        );
    }

    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;
    let total_bytes = response.content_length();
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(dest).await?;
    let mut downloaded = 0u64;
    let mut last_emit_bytes = 0u64;

    while let Some(chunk) = stream.next().await {
        if let Some(p) = progress {
            p.check_cancelled()?;
        }

        let chunk = chunk?;
        downloaded += chunk.len() as u64;
        file.write_all(&chunk).await?;

        if let Some(p) = progress {
            let should_emit = downloaded - last_emit_bytes >= 65536
                || total_bytes.map(|t| downloaded >= t).unwrap_or(false);
            if should_emit {
                last_emit_bytes = downloaded;
                p.emit_download(
                    opts.stage,
                    opts.percent_base,
                    opts.percent_span,
                    &display_label,
                    &opts.file_name,
                    opts.item_index,
                    opts.item_total,
                    downloaded,
                    total_bytes,
                );
            }
        }
    }

    file.flush().await?;

    if let Some(p) = progress {
        p.emit_download(
            opts.stage,
            opts.percent_base,
            opts.percent_span,
            &display_label,
            &opts.file_name,
            opts.item_index,
            opts.item_total,
            downloaded,
            total_bytes.or(Some(downloaded)),
        );
        p.log(format!("Done: {}", opts.file_name));
    }

    Ok(())
}

pub fn maven_path(base: &Path, name: &str) -> std::path::PathBuf {
    maven_path_with_classifier(base, name)
}

pub fn maven_path_with_classifier(base: &Path, name: &str) -> std::path::PathBuf {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return base.to_path_buf();
    }
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    let classifier = parts.get(3).filter(|c| !c.is_empty());
    if [group, artifact, version]
        .iter()
        .any(|p| p.is_empty() || *p == ".." || p.contains('/') || p.contains('\\'))
    {
        return base.to_path_buf();
    }
    let group_path = group.replace('.', "/");
    let file = if let Some(classifier) = classifier {
        format!("{artifact}-{version}-{classifier}.jar")
    } else {
        format!("{artifact}-{version}.jar")
    };
    base.join(group_path)
        .join(artifact)
        .join(version)
        .join(file)
}

pub fn library_artifact_path(libs_dir: &Path, name: &str, artifact_path: Option<&str>) -> PathBuf {
    if let Some(path) = artifact_path {
        if let Ok(joined) = crate::services::paths::join_contained(libs_dir, path) {
            return joined;
        }
    }
    maven_path_with_classifier(libs_dir, name)
}

pub fn is_native_library_name(name: &str) -> bool {
    name.contains(":natives-")
}

pub fn library_allowed_os(rules: Option<&Vec<crate::services::mojang::LibraryRule>>) -> bool {
    if let Some(rules) = rules {
        let os = std::env::consts::OS;
        let mut allow = false;
        for rule in rules {
            let os_name = rule.os.as_ref().and_then(|o| o.name.as_deref());
            let matches = match os_name {
                Some("linux") => os == "linux",
                Some("osx") => os == "macos",
                Some("windows") => os == "windows",
                _ => true,
            };
            if matches {
                allow = rule.action == "allow";
            }
        }
        if !allow {
            return false;
        }
    }
    true
}
