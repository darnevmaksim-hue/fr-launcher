use crate::error::{err, Result};
use crate::models::{
    AccountsStore, AuthAccount, BackgroundAsset, ChangelogFeed, DeviceCodeSession, GameLogEntry,
    HorizonConfig, InstalledMod, JavaInstallation, LauncherSettings, LauncherUpdateInfo,
    McVersionGroup, McVersionSupport, ModBrowsePage, ModDownloadPlanItem, ModDownloadSelection,
    ModSearchResult, ModVersionInfo, ModrinthRelease, Profile,
};
use crate::services::{
    auth, backgrounds, changelog, config, curseforge, download, export, installer, java, launcher,
    mod_deps, modrinth, mods, paths, profiles, skin_avatar, versions,
};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn get_supported_versions() -> Vec<McVersionSupport> {
    versions::supported_versions()
}

#[tauri::command]
pub fn get_supported_version_groups() -> Vec<McVersionGroup> {
    versions::supported_version_groups()
}

#[tauri::command]
pub fn get_launcher_settings() -> Result<LauncherSettings> {
    profiles::get_settings()
}

#[tauri::command]
pub fn curseforge_available() -> bool {
    curseforge::has_api_key()
}

#[tauri::command]
pub fn save_launcher_settings(settings: LauncherSettings) -> Result<()> {
    profiles::save_settings(&settings)
}

#[tauri::command]
pub fn list_profiles() -> Result<Vec<Profile>> {
    profiles::list()
}

#[tauri::command]
pub fn create_profile(
    name: String,
    mc_version: String,
    loader: String,
    ram_mb: Option<u32>,
    category: Option<String>,
) -> Result<Profile> {
    let ram = ram_mb.unwrap_or_else(|| {
        profiles::get_settings()
            .map(|s| s.default_ram_mb)
            .unwrap_or(4096)
    });
    profiles::create(name, mc_version, loader, ram, category)
}

#[tauri::command]
pub fn update_profile(profile: Profile) -> Result<Profile> {
    profiles::update(profile)
}

#[tauri::command]
pub fn delete_profile(profile_id: String) -> Result<()> {
    profiles::delete(&profile_id)
}

#[tauri::command]
pub async fn detect_java_installations() -> Vec<JavaInstallation> {
    tokio::task::spawn_blocking(java::detect_all)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub fn read_horizon_config(profile_id: String) -> Result<HorizonConfig> {
    config::read(&profile_id)
}

#[tauri::command]
pub fn write_horizon_config(profile_id: String, config_data: HorizonConfig) -> Result<()> {
    config::write(&profile_id, &config_data)
}

#[tauri::command]
pub async fn resolve_account_avatar(account: AuthAccount, size: Option<u32>) -> Result<String> {
    skin_avatar::ensure_avatar(&account, size.unwrap_or(32)).await
}

#[tauri::command]
pub fn open_instance_folder(profile_id: String, app: AppHandle) -> Result<()> {
    let path = paths::instance_dir(&profile_id);
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    app.opener()
        .open_path(path.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| err(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn open_mods_folder(profile_id: String, app: AppHandle) -> Result<()> {
    let path = paths::mods_dir(&profile_id);
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    app.opener()
        .open_path(path.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| err(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn cancel_install() {
    download::cancel_install();
}

#[tauri::command]
pub async fn install_instance(profile_id: String, app: AppHandle) -> Result<Profile> {
    let profile = profiles::get(&profile_id)?;
    installer::install_profile(&profile, app).await?;
    profiles::get(&profile_id)
}

#[tauri::command]
pub async fn launch_instance(profile_id: String, app: AppHandle) -> Result<u32> {
    let profile = profiles::get(&profile_id)?;
    launcher::launch(&profile, app, None).await
}

#[tauri::command]
pub fn kill_game() -> Result<()> {
    launcher::kill_game()
}

#[tauri::command]
pub fn is_game_running() -> bool {
    launcher::is_game_running()
}

#[tauri::command]
pub async fn fetch_horizonui_release_info(
    mc_version: String,
    loader: String,
) -> Result<ModrinthRelease> {
    modrinth::fetch_release(&mc_version, &loader).await
}

#[tauri::command]
pub async fn fetch_changelog() -> Result<ChangelogFeed> {
    changelog::fetch_changelog().await
}

#[tauri::command]
pub async fn check_launcher_update() -> Result<Option<LauncherUpdateInfo>> {
    let current = env!("CARGO_PKG_VERSION");
    let release = changelog::check_launcher_update(current).await?;
    Ok(release.map(|r| LauncherUpdateInfo {
        version: r.tag_name,
        url: r.html_url,
        notes: r.body,
    }))
}

#[tauri::command]
pub async fn get_account() -> Result<Option<AuthAccount>> {
    auth::account_status().await
}

#[tauri::command]
pub fn list_accounts() -> Result<AccountsStore> {
    auth::list_accounts_store()
}

#[tauri::command]
pub fn switch_account(account_id: String) -> Result<AuthAccount> {
    auth::switch_account(&account_id)
}

#[tauri::command]
pub fn remove_account(account_id: String) -> Result<()> {
    auth::remove_account(&account_id)
}

#[tauri::command]
pub fn cancel_login() {
    auth::cancel_login();
}

#[tauri::command]
pub async fn login_microsoft_browser(app: AppHandle) -> Result<AuthAccount> {
    auth::login_microsoft_browser(|url| {
        app.opener()
            .open_url(&url, None::<&str>)
            .map_err(|e| err(e.to_string()))
    })
    .await
}

#[tauri::command]
pub async fn login_elyby_browser(app: AppHandle) -> Result<AuthAccount> {
    auth::login_elyby_browser(|url| {
        app.opener()
            .open_url(&url, None::<&str>)
            .map_err(|e| err(e.to_string()))
    })
    .await
}

#[tauri::command]
pub async fn begin_ely_login() -> Result<DeviceCodeSession> {
    auth::begin_ely_device_login().await
}

#[tauri::command]
pub async fn complete_ely_login(session: DeviceCodeSession) -> Result<AuthAccount> {
    auth::complete_ely_device_login(session).await
}

#[tauri::command]
pub async fn begin_microsoft_login() -> Result<DeviceCodeSession> {
    auth::begin_device_login().await
}

#[tauri::command]
pub async fn complete_microsoft_login(session: DeviceCodeSession) -> Result<AuthAccount> {
    auth::complete_device_login(session).await
}

#[tauri::command]
pub async fn login_elyby(
    username: String,
    password: String,
    totp: Option<String>,
) -> Result<AuthAccount> {
    auth::login_elyby(username, password, totp).await
}

#[tauri::command]
pub fn login_offline(username: String) -> Result<AuthAccount> {
    auth::login_offline(username)
}

#[tauri::command]
pub fn logout_account() -> Result<()> {
    auth::logout()
}

#[tauri::command]
pub async fn list_backgrounds(profile_id: String) -> Result<Vec<BackgroundAsset>> {
    tokio::task::spawn_blocking(move || backgrounds::list_backgrounds(&profile_id))
        .await
        .map_err(|e| err(e.to_string()))?
}

#[tauri::command]
pub fn read_local_image(path: String) -> Result<Vec<u8>> {
    backgrounds::read_image_bytes(&path)
}

#[tauri::command]
pub async fn export_profiles(app: AppHandle) -> Result<String> {
    let dest = export::default_export_path();
    export::export_bundle(&dest)?;
    app.dialog()
        .message(format!("Exported to {}", dest.display()))
        .title("Export complete")
        .show(|_| {});
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn import_profiles(app: AppHandle) -> Result<u32> {
    let picked = app
        .dialog()
        .file()
        .add_filter("Zip", &["zip"])
        .blocking_pick_file();

    let path = picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| err("No file selected"))?;

    let count = export::import_bundle(&path)?;
    Ok(count)
}

#[tauri::command]
pub fn open_url(url: String, app: AppHandle) -> Result<()> {
    crate::services::download::assert_http_url(&url)?;
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| err(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn log_dir() -> String {
    paths::logs_dir().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn log_file() -> String {
    paths::log_file().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn open_log_folder(app: AppHandle) -> Result<()> {
    let path = paths::logs_dir();
    std::fs::create_dir_all(&path)?;
    log::info!("Opening log folder {}", path.display());
    app.opener()
        .open_path(path.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| err(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn list_installed_mods(profile_id: String) -> Result<Vec<InstalledMod>> {
    mods::list_installed(&profile_id)
}

#[tauri::command]
pub async fn browse_mods(
    profile_id: String,
    query: String,
    source: String,
    offset: u32,
    limit: u32,
    sort: String,
) -> Result<ModBrowsePage> {
    let profile = profiles::get(&profile_id)?;
    let page_limit = limit.clamp(1, 50);
    match source.as_str() {
        "modrinth" => {
            modrinth::browse(
                &profile_id,
                &query,
                &profile.loader,
                &profile.mc_version,
                offset,
                page_limit,
                &sort,
            )
            .await
        }
        "curseforge" => {
            log::info!(
                "browse_mods curseforge profile={profile_id} query={query:?} offset={offset}"
            );
            curseforge::browse(&profile_id, &query, offset, page_limit, &sort).await
        }
        _ => Err(err("Unknown mod source")),
    }
}

#[tauri::command]
pub async fn search_mods(
    profile_id: String,
    query: String,
    source: String,
) -> Result<Vec<ModSearchResult>> {
    let page = browse_mods(profile_id, query, source, 0, 25, "downloads".into()).await?;
    Ok(page.results)
}

#[tauri::command]
pub async fn get_mod_versions(
    profile_id: String,
    mod_id: String,
    source: String,
) -> Result<Vec<ModVersionInfo>> {
    let profile = profiles::get(&profile_id)?;
    match source.as_str() {
        "modrinth" => {
            modrinth::get_versions(&profile_id, &mod_id, &profile.loader, &profile.mc_version).await
        }
        "curseforge" => curseforge::get_versions(&profile_id, &mod_id).await,
        _ => Err(err("Unknown mod source")),
    }
}

#[tauri::command]
pub async fn install_mod(profile_id: String, download_url: String, filename: String) -> Result<()> {
    mods::install_from_url(&profile_id, &filename, &download_url).await
}

#[tauri::command]
pub async fn plan_mod_downloads(
    profile_id: String,
    selections: Vec<ModDownloadSelection>,
) -> Result<Vec<ModDownloadPlanItem>> {
    mod_deps::plan_downloads(&profile_id, selections).await
}

#[tauri::command]
pub async fn install_mods_batch(
    profile_id: String,
    items: Vec<ModDownloadPlanItem>,
) -> Result<usize> {
    mod_deps::install_plan(&profile_id, &items).await
}

#[tauri::command]
pub fn remove_mod(profile_id: String, filename: String) -> Result<()> {
    mods::remove(&profile_id, &filename)
}

#[tauri::command]
pub fn read_game_log(profile_id: String, max_lines: Option<u32>) -> Result<Vec<GameLogEntry>> {
    let path = paths::game_dir(&profile_id).join("logs").join("latest.log");
    if !path.is_file() {
        return Ok(vec![]);
    }

    let content = std::fs::read_to_string(&path)?;
    let max = max_lines.unwrap_or(10_000) as usize;
    let lines: Vec<GameLogEntry> = content
        .lines()
        .rev()
        .take(max)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|line| GameLogEntry {
            message: line.to_string(),
            level: game_log_level(line),
        })
        .collect();

    Ok(lines)
}

fn game_log_level(line: &str) -> String {
    let upper = line.to_uppercase();
    if upper.contains(" ERROR") || upper.contains("[ERROR]") {
        "error".into()
    } else if upper.contains(" WARN") || upper.contains("[WARN]") {
        "warn".into()
    } else {
        "info".into()
    }
}
