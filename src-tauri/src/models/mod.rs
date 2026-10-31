use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McVersionSupport {
    pub version: String,
    pub loaders: Vec<String>,
    pub java: u8,
    pub mod_version: String,
    pub group_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McVersionGroup {
    pub id: String,
    pub label: String,
    pub mod_version: String,
    pub versions: Vec<String>,
    pub loaders: Vec<String>,
    pub java: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: String,
    pub ram_mb: u32,
    pub java_path: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub last_played: Option<String>,
    #[serde(default)]
    pub play_time_ms: u64,
    pub installed: bool,
    #[serde(default)]
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSettings {
    pub default_ram_mb: u32,
    pub close_on_launch: bool,
    pub theme: String,
    #[serde(default)]
    pub curseforge_api_key: Option<String>,
    #[serde(default)]
    pub microsoft_oauth_client_id: Option<String>,
    #[serde(default)]
    pub ely_oauth_client_id: Option<String>,
    #[serde(default)]
    pub ely_oauth_client_secret: Option<String>,
    #[serde(default)]
    pub animated_backgrounds: bool,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            default_ram_mb: 4096,
            close_on_launch: false,
            theme: "horizon".into(),
            curseforge_api_key: None,
            microsoft_oauth_client_id: None,
            ely_oauth_client_id: None,
            ely_oauth_client_secret: None,
            animated_backgrounds: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstallation {
    pub path: String,
    pub version: u8,
    pub vendor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HorizonConfig {
    pub background_frame: i32,
    pub background: String,
    pub agreed_to_terms: bool,
    pub auto_updater: bool,
    pub discord_rpc: HorizonDiscordRpc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HorizonDiscordRpc {
    pub enabled: bool,
    pub update_interval: i32,
    pub display: HorizonDisplayOptions,
    pub privacy: HorizonPrivacySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HorizonDisplayOptions {
    pub show_health: bool,
    pub show_armor: bool,
    pub show_level: bool,
    pub show_time: bool,
    pub show_biome: bool,
    pub show_dimension: bool,
    pub show_party: bool,
    pub show_head: bool,
    pub dynamic_images: bool,
    pub use_templates: bool,
    pub active_template: String,
    pub time_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HorizonPrivacySettings {
    pub show_coordinates: bool,
    pub show_server_ip: bool,
    pub show_server_name: bool,
    pub show_player_count: bool,
    pub show_world_name: bool,
    pub obfuscate_in_hardcore: bool,
    pub hide_while_afk: bool,
    pub auto_detect_activity: bool,
    pub detect_afk: bool,
    pub auto_detection: i32,
    pub afk_timeout: i32,
    pub coordinate_precision: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub stage: String,
    pub percent: u8,
    pub message: String,
    pub current_file: Option<String>,
    pub item_index: Option<u32>,
    pub item_total: Option<u32>,
    pub bytes_downloaded: Option<u64>,
    pub bytes_total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallLogEntry {
    pub message: String,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLogEntry {
    pub message: String,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthRelease {
    pub version: String,
    pub version_id: String,
    pub filename: String,
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDownloadSelection {
    pub mod_id: String,
    pub version_id: String,
    pub source: String,
    pub title: String,
    pub version_name: String,
    pub filename: String,
    pub download_url: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDownloadPlanItem {
    pub mod_id: String,
    pub version_id: String,
    pub source: String,
    pub title: String,
    pub version_name: String,
    pub filename: String,
    pub download_url: String,
    pub size_bytes: u64,
    pub required_by: Option<String>,
    pub is_dependency: bool,
    pub already_installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub filename: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSearchResult {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub author: String,
    pub downloads: u64,
    pub source: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModBrowsePage {
    pub results: Vec<ModSearchResult>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModVersionFile {
    pub filename: String,
    pub download_url: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModVersionInfo {
    pub id: String,
    pub name: String,
    pub files: Vec<ModVersionFile>,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogEntry {
    pub mc_version: String,
    pub version: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogFeed {
    pub homepage: String,
    pub entries: Vec<ChangelogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundAsset {
    pub name: String,
    pub path: Option<String>,
    #[serde(default)]
    pub frames: Vec<String>,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherUpdateInfo {
    pub version: String,
    pub url: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthAccount {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    #[serde(default)]
    pub client_token: Option<String>,
}

fn default_auth_type() -> String {
    "microsoft".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsStore {
    pub accounts: Vec<AuthAccount>,
    pub active_account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeSession {
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    #[serde(default)]
    pub message: String,
    pub device_code: String,
    pub interval: u32,
    pub expires_in: u32,
}

impl Default for HorizonConfig {
    fn default() -> Self {
        Self {
            background_frame: 20,
            background: "default".into(),
            agreed_to_terms: false,
            auto_updater: true,
            discord_rpc: HorizonDiscordRpc {
                enabled: true,
                update_interval: 1000,
                display: HorizonDisplayOptions {
                    show_health: true,
                    show_armor: true,
                    show_level: true,
                    show_time: true,
                    show_biome: true,
                    show_dimension: true,
                    show_party: true,
                    show_head: true,
                    dynamic_images: true,
                    use_templates: false,
                    active_template: "detailed".into(),
                    time_format: "ELAPSED".into(),
                },
                privacy: HorizonPrivacySettings {
                    show_coordinates: false,
                    show_server_ip: false,
                    show_server_name: true,
                    show_player_count: false,
                    show_world_name: true,
                    obfuscate_in_hardcore: true,
                    hide_while_afk: false,
                    auto_detect_activity: true,
                    detect_afk: true,
                    auto_detection: 1000,
                    afk_timeout: 300_000,
                    coordinate_precision: 2,
                },
            },
        }
    }
}
