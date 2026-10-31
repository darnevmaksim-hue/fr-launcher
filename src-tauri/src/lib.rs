pub mod api;
pub mod commands;
pub mod error;
pub mod models;
pub mod services;

use commands::*;
use tauri::image::Image;
use tauri::Manager;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

const APP_ICON_PNG: &[u8] = include_bytes!("../icons/icon.png");

fn apply_window_icon(app: &tauri::App) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Ok(icon) = Image::from_bytes(APP_ICON_PNG) {
        let _ = window.set_icon(icon);
    } else if let Some(icon) = app.default_window_icon() {
        let _ = window.set_icon(icon.clone());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = std::fs::create_dir_all(services::paths::logs_dir());
    let _ = services::paths::ensure_launcher_dirs();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .rotation_strategy(RotationStrategy::KeepOne)
                .max_file_size(8 * 1024 * 1024)
                .level(log::LevelFilter::Debug)
                .level_for("reqwest", log::LevelFilter::Info)
                .level_for("hyper", log::LevelFilter::Warn)
                .level_for("hyper_util", log::LevelFilter::Warn)
                .level_for("rustls", log::LevelFilter::Warn)
                .level_for("native_tls", log::LevelFilter::Warn)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::Folder {
                        path: services::paths::logs_dir(),
                        file_name: Some("fr-launcher".into()),
                    }),
                ])
                .build(),
        )
        .setup(|app| {
            log::info!(
                "FR-Launcher {} starting; data {} game {}",
                env!("CARGO_PKG_VERSION"),
                services::paths::data_root().display(),
                services::paths::game_root().display()
            );
            apply_window_icon(app);

            // Тестовый режим: FR_TEST_MODPACK=<slug> — импортировать модпак с Modrinth.
            if let Ok(slug) = std::env::var("FR_TEST_MODPACK") {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let args = vec![serde_json::json!(slug), serde_json::Value::Null];
                    match crate::api::api("modpack:install".into(), args, handle.clone()).await {
                        Ok(v) => log::info!("fr-test: modpack ok: {v}"),
                        Err(e) => log::error!("fr-test: modpack failed: {e}"),
                    }
                });
            }

            // Тестовый режим: FR_TEST_LAUNCH=<instance_id> — установить и запустить инстанс.
            if let Ok(instance_id) = std::env::var("FR_TEST_LAUNCH") {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let args = vec![serde_json::json!({ "instanceId": instance_id })];
                    match crate::api::api("game:launch".into(), args, handle.clone()).await {
                        Ok(v) => log::info!("fr-test: launch ok: {v}"),
                        Err(e) => log::error!("fr-test: launch failed: {e}"),
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::api::api,
            get_supported_versions,
            get_supported_version_groups,
            get_launcher_settings,
            save_launcher_settings,
            curseforge_available,
            list_profiles,
            create_profile,
            update_profile,
            delete_profile,
            detect_java_installations,
            read_horizon_config,
            write_horizon_config,
            resolve_account_avatar,
            open_instance_folder,
            open_mods_folder,
            cancel_install,
            install_instance,
            kill_game,
            is_game_running,
            launch_instance,
            fetch_horizonui_release_info,
            fetch_changelog,
            check_launcher_update,
            get_account,
            list_accounts,
            switch_account,
            remove_account,
            cancel_login,
            login_microsoft_browser,
            login_elyby_browser,
            begin_microsoft_login,
            complete_microsoft_login,
            begin_ely_login,
            complete_ely_login,
            login_elyby,
            login_offline,
            logout_account,
            list_backgrounds,
            read_local_image,
            export_profiles,
            import_profiles,
            open_url,
            list_installed_mods,
            browse_mods,
            search_mods,
            get_mod_versions,
            install_mod,
            plan_mod_downloads,
            install_mods_batch,
            remove_mod,
            read_game_log,
            log_dir,
            log_file,
            open_log_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
