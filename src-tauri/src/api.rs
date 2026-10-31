//! Единая точка входа для интерфейса FR-Launcher.
//!
//! Фронтенд (ui/) — это наш родной интерфейс из Electron-версии. Вместо
//! contextBridge он получает объект `window.API`, каждый метод которого
//! вызывает команду `api` с именем IPC-канала и аргументами. Здесь каналы
//! реализуются поверх Rust-ядра (за основу взяты сервисы HorizonUI, GPL-3.0).

use crate::error::{err, Result};
use crate::services::{auth, installer, instance_meta, java, launcher, paths, store};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;

fn arg(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or(Value::Null)
}

fn sarg(args: &[Value], i: usize) -> String {
    args.get(i)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn narg(args: &[Value], i: usize) -> u64 {
    args.get(i).and_then(|v| v.as_u64()).unwrap_or(0)
}

async fn get_json(url: &str) -> Result<Value> {
    let client = reqwest::Client::builder()
        .user_agent("FR-Launcher/1.0 (fr-launcher)")
        .timeout(Duration::from_secs(30))
        .build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(err(format!("HTTP {} for {url}", resp.status())));
    }
    Ok(resp.json().await?)
}

fn emit_log(app: &AppHandle, text: impl Into<String>, cls: &str) {
    let text = text.into();
    log::info!("{}", text.trim_end());
    let _ = app.emit("game:log", json!({ "text": text, "cls": cls }));
}

/// Минимальная версия Java для версии Minecraft.
fn required_java(mc: &str) -> u8 {
    let mut parts = mc.split('.');
    let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    if major >= 26 {
        25
    } else if major == 1 && minor >= 21 {
        21
    } else if major == 1 && minor >= 17 {
        17
    } else {
        8
    }
}

fn java_major_of(path: &str) -> Option<u8> {
    java::detect_all()
        .into_iter()
        .find(|j| j.path.eq_ignore_ascii_case(path))
        .map(|j| j.version)
}

fn find_java(required: u8) -> Option<String> {
    let mut list: Vec<_> = java::detect_all()
        .into_iter()
        .filter(|j| j.version >= required)
        .collect();
    // Предпочитаем 17/21, а не самые новые сборки.
    list.sort_by_key(|j| if j.version <= 21 { j.version as i32 } else { 100 - j.version as i32 });
    list.pop().map(|j| j.path)
}

/// Запуск установки + игры. Возвращает pid java-процесса.
async fn launch_instance(args: &[Value], app: AppHandle) -> Result<Value> {
    let opts = arg(args, 0);
    let id = opts
        .get("instanceId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if id.is_empty() {
        return Err(err("instanceId is required"));
    }
    let inst = store::get(&id).ok_or_else(|| err("Instance not found"))?;

    // Настройки, изменённые в интерфейсе, сохраняем перед запуском.
    let ram = opts.get("ram").and_then(|v| v.as_u64()).map(|v| v as u32);
    let java_path = opts.get("javaPath").and_then(|v| v.as_str()).unwrap_or("");
    if ram.is_some() || (inst.java_path.is_empty() && !java_path.is_empty()) {
        let _ = store::update(&id, |i| {
            if let Some(r) = ram {
                i.ram = r;
            }
            if i.java_path.is_empty() && !java_path.is_empty() {
                i.java_path = java_path.to_string();
            }
        });
    }

    log::info!(
        "launch: {} | MC {} | {} | java {}",
        inst.name,
        inst.mc_version,
        inst.loader,
        inst.java_path
    );

    // Java: если выбранная не подходит по версии — подбираем подходящую.
    let required = required_java(&inst.mc_version);
    let current = store::get(&id).map(|i| i.java_path).unwrap_or_default();
    let current_ok = !current.is_empty()
        && java_major_of(&current)
            .map(|v| v >= required)
            .unwrap_or(false);
    if !current_ok {
        if let Some(p) = find_java(required) {
            log::info!("launch: выбираю Java {p} (нужна {required}+)");
            let _ = store::update(&id, |i| i.java_path = p.clone());
        } else {
            log::warn!("launch: Java {required}+ не найдена");
        }
    }

    // Подхватываем уже установленную версию (например, из Electron-версии).
    let loader_key = store::loader_key(&inst.loader);
    let mut profile = store::to_profile(&store::get(&id).unwrap_or(inst.clone()));
    if !profile.installed {
        if let Some(v) = store::detect_installed_version(&inst.mc_version, &loader_key) {
            log::info!("launch: найдена готовая версия {v}, установка не нужна");
            instance_meta::save_instance_meta(&id, &loader_key, &v)?;
            store::mark_installed(&id, &v)?;
            profile = store::to_profile(&store::get(&id).ok_or_else(|| err("Instance lost"))?);
        }
    }

    if !profile.installed {
        emit_log(&app, "[FR] Установка инстанса…\n", "ok");
        installer::install_profile(&profile, app.clone())
            .await
            .map_err(|e| {
                log::error!("install failed: {e}");
                e
            })?;
        let meta = instance_meta::load_instance_meta(&id)?;
        store::mark_installed(&id, &meta.version_id)?;
    }

    let profile = store::to_profile(&store::get(&id).ok_or_else(|| err("Instance lost"))?);
    let username = opts
        .get("username")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string());
    let pid = launcher::launch(&profile, app.clone(), username).await?;

    // Учёт игрового времени: пишем в наш stats.json по завершении процесса.
    let app_exit = app.clone();
    let id_exit = id.clone();
    let started = std::time::Instant::now();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if !launcher::is_game_running() {
                break;
            }
        }
        let seconds = started.elapsed().as_secs();
        record_playtime(&id_exit, seconds);
        let _ = app_exit.emit("stats:changed", json!({}));
    });

    Ok(json!({ "ok": true, "pid": pid }))
}

fn stats_path() -> std::path::PathBuf {
    paths::data_root().join("stats.json")
}

fn load_stats() -> Value {
    std::fs::read_to_string(stats_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

fn record_playtime(instance_id: &str, seconds: u64) {
    let mut stats = load_stats();
    if !stats.is_object() {
        stats = json!({});
    }
    let obj = stats.as_object_mut().unwrap();
    let entry = obj
        .entry(instance_id.to_string())
        .or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    let cur_seconds = entry.get("seconds").and_then(|v| v.as_u64()).unwrap_or(0);
    let cur_sessions = entry.get("sessions").and_then(|v| v.as_u64()).unwrap_or(0);
    entry["seconds"] = json!(cur_seconds + seconds);
    entry["sessions"] = json!(cur_sessions + 1);
    entry["last"] = json!(chrono::Utc::now().timestamp_millis());
    let _ = std::fs::write(
        stats_path(),
        serde_json::to_string_pretty(&stats).unwrap_or_default(),
    );
}

fn backgrounds_dir() -> std::path::PathBuf {
    paths::data_root().join("backgrounds")
}

fn find_bg() -> Option<std::path::PathBuf> {
    ["png", "jpg", "jpeg", "webp", "bmp", "gif"]
        .iter()
        .map(|ext| backgrounds_dir().join(format!("bg.{ext}")))
        .find(|p| p.exists())
}

fn bg_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        _ => "image/png",
    }
}

fn get_bg() -> Value {
    let Some(path) = find_bg() else {
        return Value::Null;
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return Value::Null;
    };
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    json!({
        "path": path.to_string_lossy(),
        "dataUrl": format!("data:{};base64,{}", bg_mime(&path), data),
    })
}

async fn pick_file(app: &AppHandle, title: &str) -> Option<std::path::PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title(title)
        .pick_file(move |p| {
            let _ = tx.send(p);
        });
    rx.await
        .ok()
        .flatten()
        .map(|p| std::path::PathBuf::from(p.to_string()))
}

async fn modrinth_search(query: &str, opts: &Value) -> Result<Value> {
    let limit = opts.get("limit").and_then(|v| v.as_u64()).unwrap_or(12);
    let index = opts
        .get("index")
        .and_then(|v| v.as_str())
        .unwrap_or("relevance");
    let mut url = format!(
        "https://api.modrinth.com/v2/search?query={}&limit={}&index={}",
        urlencode(query),
        limit,
        index
    );
    if let Some(offset) = opts.get("offset").and_then(|v| v.as_u64()) {
        url.push_str(&format!("&offset={offset}"));
    }
    if let Some(facets) = opts.get("facets") {
        if !facets.is_null() {
            url.push_str(&format!("&facets={}", urlencode(&facets.to_string())));
        }
    }
    let data = get_json(&url).await?;
    Ok(data.get("hits").cloned().unwrap_or_else(|| json!([])))
}

fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

async fn modrinth_versions(slug: &str, mc: &str, loader: &str, kind: &str) -> Result<Value> {
    let mut url = format!(
        "https://api.modrinth.com/v2/project/{}/version",
        urlencode(slug)
    );
    let mut params: Vec<String> = Vec::new();
    if !mc.is_empty() {
        params.push(format!("game_versions={}", urlencode(&json!([mc]).to_string())));
    }
    if !loader.is_empty() {
        params.push(format!("loaders={}", urlencode(&json!([loader]).to_string())));
    }
    if kind == "modpack" {
        params.push(format!(
            "featured={}",
            urlencode(&json!([true]).to_string())
        ));
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }
    get_json(&url).await
}

async fn download_file(url: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let client = reqwest::Client::builder()
        .user_agent("FR-Launcher/1.0 (fr-launcher)")
        .timeout(Duration::from_secs(600))
        .build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(err(format!("Download failed: HTTP {}", resp.status())));
    }
    let bytes = resp.bytes().await?;
    std::fs::write(dest, &bytes)?;
    Ok(())
}

fn instance_by_id(id: &str) -> Result<store::Instance> {
    store::get(id).ok_or_else(|| err("Instance not found"))
}

#[tauri::command]
pub async fn api(method: String, args: Vec<Value>, app: AppHandle) -> Result<Value> {
    log::debug!("api: {method}");
    match method.as_str() {
        "app:info" => Ok(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "platform": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "portable": true,
        })),

        "fs:exists" => Ok(json!(Path::new(&sarg(&args, 0)).exists())),

        "dialog:open" => {
            let title = arg(&args, 0)
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Выберите файл")
                .to_string();
            Ok(match pick_file(&app, &title).await {
                Some(p) => json!(p.to_string_lossy()),
                None => Value::Null,
            })
        }

        // ===== Java =====
        "java:detect" => {
            let required = narg(&args, 0).max(1) as u8;
            let mut list = java::detect_all();
            list.sort_by(|a, b| b.version.cmp(&a.version));
            let installations: Vec<Value> = list
                .iter()
                .filter(|j| j.version >= required)
                .map(|j| {
                    json!({
                        "path": j.path,
                        "version": j.version,
                        "vendor": j.vendor,
                        "major": j.version,
                    })
                })
                .collect();
            Ok(json!(installations))
        }
        "java:ensure" => {
            let mut list = java::detect_all();
            list.sort_by(|a, b| b.version.cmp(&a.version));
            let best = list
                .iter()
                .filter(|j| j.version >= 17)
                .max_by_key(|j| {
                    // Предпочитаем 17/21, а не самые новые сборки.
                    if j.version <= 21 {
                        j.version as i32
                    } else {
                        100 - j.version as i32
                    }
                });
            Ok(match best {
                Some(j) => json!({
                    "path": j.path,
                    "raw": format!("{} ({})", j.vendor, j.version),
                    "system": true,
                    "major": j.version,
                    "version": j.version,
                }),
                None => json!({ "error": "Java 17+ не найдена" }),
            })
        }

        // ===== Minecraft =====
        "mc:versions" => {
            let data =
                get_json("https://launchermeta.mojang.com/mc/game/version_manifest_v2.json").await?;
            let versions: Vec<Value> = data
                .get("versions")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|v| {
                    json!({
                        "id": v.get("id").cloned().unwrap_or(Value::Null),
                        "type": v.get("type").cloned().unwrap_or(Value::Null),
                        "releaseTime": v.get("releaseTime").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect();
            Ok(json!(versions))
        }
        "mc:dir" => Ok(json!(paths::game_root().to_string_lossy())),

        "loader:fabric:versions" => {
            let mc = sarg(&args, 0);
            let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc}");
            let data = get_json(&url).await?;
            let list: Vec<String> = data
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .take(20)
                .filter_map(|v| {
                    v.get("loader")
                        .and_then(|l| l.get("version"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
            Ok(json!(list))
        }

        // ===== Instances =====
        "instances:list" => Ok(json!(store::list())),
        "instances:create" => Ok(serde_json::to_value(store::create(&arg(&args, 0))?)?),
        "instances:delete" => {
            store::delete(&sarg(&args, 0))?;
            Ok(Value::Null)
        }
        "instances:dir" => {
            let id = sarg(&args, 0);
            if id.is_empty() {
                return Ok(Value::Null);
            }
            Ok(json!(paths::instance_dir(&id).to_string_lossy()))
        }
        "instances:saveMods" => {
            let id = sarg(&args, 0);
            let mods = arg(&args, 1).as_array().cloned().unwrap_or_default();
            store::save_mods(&id, mods)
        }
        "instance:mods" => Ok(json!(store::mod_files(&sarg(&args, 0)))),
        "instance:modDelete" => Ok(json!(store::delete_mod_by_slug(
            &sarg(&args, 0),
            &sarg(&args, 1)
        ))),
        "instance:modDeleteFile" => Ok(json!(store::delete_mod_file(
            &sarg(&args, 0),
            &sarg(&args, 1)
        ))),
        "instance:files" => Ok(json!(store::files_in(&sarg(&args, 0), &sarg(&args, 1)))),
        "instance:fileDelete" => Ok(json!(store::delete_file_in(
            &sarg(&args, 0),
            &sarg(&args, 1),
            &sarg(&args, 2)
        ))),

        // ===== Modrinth =====
        "modrinth:search" => {
            let query = sarg(&args, 0);
            let opts = arg(&args, 1);
            match modrinth_search(&query, &opts).await {
                Ok(v) => Ok(v),
                Err(e) => {
                    log::warn!("modrinth search failed: {e}");
                    Ok(Value::Null)
                }
            }
        }
        "modrinth:project" => {
            let slug = sarg(&args, 0);
            let url = format!("https://api.modrinth.com/v2/project/{}", urlencode(&slug));
            match get_json(&url).await {
                Ok(v) => Ok(v),
                Err(e) => {
                    log::warn!("modrinth project failed: {e}");
                    Ok(Value::Null)
                }
            }
        }
        "modrinth:versions" => {
            let slug = sarg(&args, 0);
            let mc = sarg(&args, 1);
            let loader = sarg(&args, 2);
            let kind = sarg(&args, 3);
            match modrinth_versions(&slug, &mc, &loader, &kind).await {
                Ok(v) => Ok(v),
                Err(e) => {
                    log::warn!("modrinth versions failed: {e}");
                    Ok(Value::Null)
                }
            }
        }

        // Прямая установка файла по URL в папку mods инстанса.
        "mod:installDirect" => {
            let data = arg(&args, 0);
            let id = data
                .get("instanceId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| err("instanceId is required"))?;
            let url = data
                .get("url")
                .or_else(|| data.get("downloadUrl"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| err("url is required"))?;
            let filename = data
                .get("filename")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    url.split('/')
                        .last()
                        .unwrap_or("mod.jar")
                        .split('?')
                        .next()
                        .unwrap_or("mod.jar")
                        .to_string()
                });
            let safe = paths::safe_filename(&filename)?;
            let dest = paths::mods_dir(id).join(&safe);
            emit_log(&app, format!("[FR] Скачивание {safe}…\n"), "ok");
            download_file(url, &dest).await?;
            emit_log(&app, format!("[FR] Установлено: {safe}\n"), "ok");
            Ok(json!({ "ok": true, "file": safe, "path": dest.to_string_lossy() }))
        }

        // Установка конкретной версии проекта Modrinth (файл + обязательные зависимости).
        "mod:install" => {
            let data = arg(&args, 0);
            let id = data
                .get("instanceId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| err("instanceId is required"))?;
            let version_id = data
                .get("versionId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| err("versionId is required"))?;
            let url = format!("https://api.modrinth.com/v2/version/{version_id}");
            let version = get_json(&url).await?;
            let mut installed = Vec::new();
            if let Some(files) = version.get("files").and_then(|f| f.as_array()) {
                if let Some(file) = files.iter().find(|f| {
                    f.get("primary").and_then(|p| p.as_bool()).unwrap_or(false)
                }).or_else(|| files.first()) {
                    let filename = file
                        .get("filename")
                        .and_then(|v| v.as_str())
                        .unwrap_or("mod.jar");
                    let download = file
                        .get("url")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| err("file url missing"))?;
                    let safe = paths::safe_filename(filename)?;
                    let dest = paths::mods_dir(id).join(&safe);
                    emit_log(&app, format!("[FR] Скачивание {safe}…\n"), "ok");
                    download_file(download, &dest).await?;
                    installed.push(safe);
                }
            }
            Ok(json!({ "ok": true, "installed": installed }))
        }

        "mod:deps" => Ok(Value::Null),
        "modpack:install" => Err(err(
            "Установка модпаков в Rust-версии ещё не перенесена",
        )),

        // ===== Launch =====
        "game:launch" => launch_instance(&args, app.clone()).await,

        // ===== Shell =====
        "open:external" => {
            let url = sarg(&args, 0);
            if url.starts_with("http://") || url.starts_with("https://") {
                let _ = open::that(&url);
            }
            Ok(Value::Null)
        }
        "open:path" => {
            let p = sarg(&args, 0);
            if !p.is_empty() && Path::new(&p).exists() {
                let _ = open::that(&p);
            }
            Ok(Value::Null)
        }

        // ===== Background =====
        "ui:getBg" => Ok(get_bg()),
        "ui:setBg" => {
            let src = sarg(&args, 0);
            let path = Path::new(&src);
            if src.is_empty() || !path.exists() {
                return Ok(json!({ "ok": false }));
            }
            std::fs::create_dir_all(backgrounds_dir())?;
            for ext in ["png", "jpg", "jpeg", "webp", "bmp", "gif"] {
                let _ = std::fs::remove_file(backgrounds_dir().join(format!("bg.{ext}")));
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .filter(|s| ["png", "jpg", "jpeg", "webp", "bmp", "gif"].contains(&s.as_str()))
                .unwrap_or_else(|| "png".into());
            let dest = backgrounds_dir().join(format!("bg.{ext}"));
            std::fs::copy(path, &dest)?;
            Ok(json!({ "ok": true, "path": dest.to_string_lossy() }))
        }
        "ui:clearBg" => {
            for ext in ["png", "jpg", "jpeg", "webp", "bmp", "gif"] {
                let _ = std::fs::remove_file(backgrounds_dir().join(format!("bg.{ext}")));
            }
            Ok(json!({ "ok": true }))
        }
        "ui:pickBg" => Ok(match pick_file(&app, "Выберите фон").await {
            Some(p) => json!(p.to_string_lossy()),
            None => Value::Null,
        }),

        // ===== Stats / metrics =====
        "stats:get" => Ok(load_stats()),
        "stats:reset" => {
            let _ = std::fs::write(stats_path(), "{}");
            Ok(json!({}))
        }
        "metrics:history" | "metrics:historyFor" => {
            Ok(json!({ "ok": true, "sessions": [], "analysis": null }))
        }

        // ===== Microsoft account =====
        "ms:status" => match auth::get_account() {
            Ok(Some(acc)) => Ok(json!({
                "mode": if acc.auth_type == "microsoft" { "msa".to_string() } else { acc.auth_type.clone() },
                "username": acc.username,
                "uuid": acc.uuid,
                "accessToken": acc.access_token,
                "skinB64": "",
            })),
            _ => Ok(Value::Null),
        },
        "ms:start" => match auth::begin_device_login().await {
            Ok(session) => Ok(json!({
                "ok": true,
                "deviceCode": session.device_code,
                "userCode": session.user_code,
                "verificationUri": session.verification_uri,
                "verificationUriComplete": session.verification_uri_complete,
                "expiresIn": session.expires_in,
                "interval": session.interval,
                "message": session.message,
            })),
            Err(e) => Ok(json!({ "ok": false, "error": e.to_string() })),
        },
        "ms:poll" => {
            let token = sarg(&args, 0);
            // В Electron сюда передавался device_code — совместимо.
            let session = crate::models::DeviceCodeSession {
                user_code: String::new(),
                verification_uri: String::new(),
                verification_uri_complete: None,
                message: String::new(),
                device_code: token,
                interval: 5,
                expires_in: 900,
            };
            match auth::poll_device_login(&session).await {
                Ok(Some(acc)) => Ok(json!({ "status": "ok", "account": acc })),
                Ok(None) => Ok(json!({ "status": "pending" })),
                Err(e) => Ok(json!({ "status": "error", "error": e.to_string() })),
            }
        }
        "ms:auth" => Ok(json!({ "ok": false, "error": "not needed" })),
        "ms:logout" => {
            let _ = auth::logout();
            let _ = app.emit("account:changed", Value::Null);
            Ok(json!({ "ok": true }))
        }

        // ===== Network =====
        "net:check" => {
            let host = sarg(&args, 0);
            let port = narg(&args, 1) as u16;
            let addr = format!("{host}:{port}");
            let ok = tokio::time::timeout(
                Duration::from_secs(3),
                tokio::net::TcpStream::connect(addr.as_str()),
            )
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false);
            Ok(json!(ok))
        }

        // ===== Ещё не перенесённые разделы (не ломаем интерфейс) =====
        "autoUpdate:check"
        | "autoUpdate:download"
        | "autoUpdate:install"
        | "autoUpdate:skip"
        | "autoUpdate:clearSkip"
        | "link:pick"
        | "link:icon"
        | "link:launch"
        | "link:tabLaunch"
        | "link:tabSwitch"
        | "link:tabClose"
        | "link:tabDetach"
        | "lans:hostInfo"
        | "lans:join"
        | "lans:leave"
        | "lans:status"
        | "sync:status"
        | "sync:enable"
        | "sync:disable"
        | "sync:pull"
        | "sync:announce"
        | "deps:analyze"
        | "repair:scan"
        | "repair:run" => Ok(Value::Null),

        "link:tabList" => Ok(json!({ "list": [] })),
        "link:stats" | "link:statsReset" => Ok(json!({})),

        other => {
            log::warn!("api: неизвестный метод {other}");
            Ok(Value::Null)
        }
    }
}
