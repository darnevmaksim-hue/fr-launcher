use crate::error::{err, Result};
use crate::models::{AccountsStore, AuthAccount, DeviceCodeSession};
use crate::services::oauth_callback;
use crate::services::paths::{account_file, accounts_file};
use crate::services::profiles;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};

static DEVICE_LOGIN_CANCELLED: AtomicBool = AtomicBool::new(false);

const MS_AUTH_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "XboxLive.signin XboxLive.offline_access";
const MS_DEVICE_LINK: &str = "https://www.microsoft.com/link";
const ELY_AUTH_URL: &str = "https://account.ely.by/oauth2/v1";
const ELY_DEVICE_CODE_URL: &str = "https://account.ely.by/api/oauth2/v1/devicecode";
const ELY_TOKEN_URL: &str = "https://account.ely.by/api/oauth2/v1/token";
const ELY_SCOPE: &str = "account_info offline_access minecraft_server_session";
const ELY_PROFILE_URL: &str = "https://account.ely.by/api/mojang/services/minecraft/profile";
const ELY_YGGDRASIL_AUTH_URL: &str = "https://authserver.ely.by/auth/authenticate";
const ELY_REFRESH_URL: &str = "https://authserver.ely.by/auth/refresh";
const ELY_DEVICE_LINK: &str = "http://account.ely.by/code";
const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const AUTH_CODE_GRANT: &str = "authorization_code";

include!(concat!(env!("OUT_DIR"), "/ely_client_id.rs"));
include!(concat!(env!("OUT_DIR"), "/ely_client_secret.rs"));
include!(concat!(env!("OUT_DIR"), "/ms_client_id.rs"));

#[derive(Deserialize)]
struct DeviceCodeResponse {
    user_code: String,
    device_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    message: Option<String>,
    interval: Option<u32>,
    expires_in: u32,
}

#[derive(Deserialize)]
struct OAuthErrorBody {
    error: Option<String>,
    message: Option<String>,
    error_message: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct XboxAuthResponse {
    Token: String,
}

#[derive(Deserialize)]
struct XstsResponse {
    Token: String,
    DisplayClaims: XstsDisplayClaims,
}

#[derive(Deserialize)]
struct XstsDisplayClaims {
    xui: Vec<XuiClaim>,
}

#[derive(Deserialize)]
struct XuiClaim {
    uhs: String,
}

#[derive(Deserialize)]
struct McAuthResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct McProfile {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct XboxErrorBody {
    #[serde(rename = "XErr")]
    xerr: Option<u64>,
    #[serde(rename = "Message")]
    message: Option<String>,
    error: Option<String>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ElyAuthResponse {
    access_token: String,
    client_token: String,
    selected_profile: ElyProfile,
}

#[derive(Deserialize)]
struct ElyProfile {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ElyErrorResponse {
    error_message: Option<String>,
    error: Option<String>,
}

pub fn cancel_login() {
    DEVICE_LOGIN_CANCELLED.store(true, Ordering::SeqCst);
    oauth_callback::cancel_oauth();
}

fn begin_login_flow() {
    DEVICE_LOGIN_CANCELLED.store(false, Ordering::SeqCst);
    oauth_callback::reset_oauth_cancel();
}

fn is_login_cancelled() -> bool {
    DEVICE_LOGIN_CANCELLED.load(Ordering::SeqCst) || oauth_callback::is_oauth_cancelled()
}

pub async fn login_microsoft_browser<F>(open_url: F) -> Result<AuthAccount>
where
    F: FnOnce(String) -> Result<()>,
{
    login_microsoft_oauth(open_url).await
}

pub async fn login_elyby_browser<F>(open_url: F) -> Result<AuthAccount>
where
    F: FnOnce(String) -> Result<()>,
{
    login_elyby_oauth(open_url).await
}

pub async fn login_microsoft_oauth<F>(open_url: F) -> Result<AuthAccount>
where
    F: FnOnce(String) -> Result<()>,
{
    begin_login_flow();
    let client_id = resolve_ms_client_id()?;
    let port = oauth_callback::reserve_oauth_port()?;
    let redirect_uri = oauth_callback::oauth_redirect_uri(port);
    let state = oauth_state();
    let auth_url = build_ms_authorize_url(MS_AUTH_URL, &client_id, &redirect_uri, SCOPE, &state);

    let listener =
        tokio::spawn(async move { oauth_callback::listen_for_oauth_code(port, state, 300).await });

    open_url(auth_url)?;

    let code = listener
        .await
        .map_err(|e| err(format!("Login listener failed: {e}")))??;

    let (ms_access, refresh) =
        exchange_oauth_code(TOKEN_URL, &client_id, None, &code, &redirect_uri).await?;
    let account = complete_xbox_chain(&ms_access, refresh).await?;
    save_account(&account)?;
    Ok(account)
}

pub async fn login_elyby_oauth<F>(open_url: F) -> Result<AuthAccount>
where
    F: FnOnce(String) -> Result<()>,
{
    begin_login_flow();
    let client_id = resolve_ely_client_id()?;
    let client_secret = resolve_ely_client_secret()?;
    let port = oauth_callback::reserve_oauth_port()?;
    let redirect_uri = oauth_callback::oauth_redirect_uri(port);
    let state = oauth_state();
    let auth_url =
        build_ely_authorize_url(ELY_AUTH_URL, &client_id, &redirect_uri, ELY_SCOPE, &state);

    let listener =
        tokio::spawn(async move { oauth_callback::listen_for_oauth_code(port, state, 300).await });

    open_url(auth_url)?;

    let code = listener
        .await
        .map_err(|e| err(format!("Login listener failed: {e}")))??;

    let (token, refresh) = exchange_oauth_code(
        ELY_TOKEN_URL,
        &client_id,
        Some(&client_secret),
        &code,
        &redirect_uri,
    )
    .await?;

    let profile = ely_minecraft_profile(&token).await?;
    let account = AuthAccount {
        username: profile.name,
        uuid: format_uuid(&profile.id),
        access_token: token,
        refresh_token: refresh,
        auth_type: "elyby".into(),
        client_token: None,
    };
    save_account(&account)?;
    Ok(account)
}

pub fn microsoft_device_link() -> &'static str {
    MS_DEVICE_LINK
}

pub fn ely_device_link() -> &'static str {
    ELY_DEVICE_LINK
}

pub async fn begin_ely_device_login() -> Result<DeviceCodeSession> {
    let client_id = resolve_ely_client_id()?;
    request_device_code(ELY_DEVICE_CODE_URL, &client_id, ELY_SCOPE).await
}

pub async fn complete_ely_device_login(session: DeviceCodeSession) -> Result<AuthAccount> {
    begin_login_flow();
    let client_id = resolve_ely_client_id()?;
    let interval = session.interval.max(5);
    let max_attempts = session.expires_in / interval + 1;
    tokio::time::sleep(std::time::Duration::from_secs(interval as u64)).await;
    for _ in 0..max_attempts {
        if is_login_cancelled() {
            return Err(err("Sign-in cancelled."));
        }
        if let Some((token, refresh)) =
            poll_device_token(ELY_TOKEN_URL, &client_id, &session.device_code).await?
        {
            let profile = ely_minecraft_profile(&token).await?;
            let account = AuthAccount {
                username: profile.name,
                uuid: format_uuid(&profile.id),
                access_token: token,
                refresh_token: refresh,
                auth_type: "elyby".into(),
                client_token: None,
            };
            save_account(&account)?;
            return Ok(account);
        }
        if is_login_cancelled() {
            return Err(err("Sign-in cancelled."));
        }
        tokio::time::sleep(std::time::Duration::from_secs(interval as u64)).await;
    }
    Err(err("Ely.by sign-in timed out. Try again."))
}

async fn request_device_code(
    device_url: &str,
    client_id: &str,
    scope: &str,
) -> Result<DeviceCodeSession> {
    let client = reqwest::Client::new();
    let response = client
        .post(device_url)
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", scope)])
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        if let Ok(err_body) = serde_json::from_str::<OAuthErrorBody>(&body) {
            let detail = err_body
                .message
                .or(err_body.error_message)
                .or(err_body.error);
            if let Some(detail) = detail {
                return Err(err(format_device_code_error(&detail, client_id)));
            }
        }
        return Err(err(
            "Could not start sign-in. Check your connection and try again.",
        ));
    }

    let res: DeviceCodeResponse = serde_json::from_str(&body)
        .map_err(|_| err("Could not read the sign-in response. Try again later."))?;

    Ok(DeviceCodeSession {
        user_code: res.user_code,
        verification_uri: res.verification_uri,
        verification_uri_complete: res.verification_uri_complete,
        message: res.message.unwrap_or_default(),
        device_code: res.device_code,
        interval: res.interval.unwrap_or(5),
        expires_in: res.expires_in,
    })
}

fn resolve_ms_client_id() -> Result<String> {
    let settings = profiles::get_settings()?;
    if let Some(id) = settings.microsoft_oauth_client_id {
        let id = id.trim();
        if !id.is_empty() {
            return Ok(id.to_string());
        }
    }
    if !embedded_ms_client_id().trim().is_empty() {
        return Ok(embedded_ms_client_id());
    }
    Err(err(
        "Microsoft OAuth is not configured. Register a HorizonUI app in Azure Portal, then set MICROSOFT_CLIENT_ID in .env or Settings → Data.",
    ))
}

fn resolve_ely_client_id() -> Result<String> {
    let settings = profiles::get_settings()?;
    if let Some(id) = settings.ely_oauth_client_id {
        let id = id.trim();
        if !id.is_empty() {
            return Ok(id.to_string());
        }
    }
    let embedded = embedded_ely_client_id();
    if !embedded.trim().is_empty() {
        return Ok(embedded);
    }
    Ok("horizonui-launcher".into())
}

fn resolve_ely_client_secret() -> Result<String> {
    let settings = profiles::get_settings()?;
    if let Some(secret) = settings.ely_oauth_client_secret {
        let secret = secret.trim();
        if !secret.is_empty() {
            return Ok(secret.to_string());
        }
    }
    let embedded = embedded_ely_client_secret();
    if !embedded.trim().is_empty() {
        return Ok(embedded);
    }
    Err(err(
        "Ely.by OAuth client secret is not configured. Register an app at account.ely.by/dev/applications with redirect http://127.0.0.1:47852/ and add the client secret in Settings → Data.",
    ))
}

fn format_device_code_error(detail: &str, client_id: &str) -> String {
    if detail.contains("invalid_client") || detail == "invalid_client" {
        return format!(
            "Ely.by rejected client ID \"{client_id}\". Check your OAuth app registration and credentials in Settings → Data."
        );
    }
    detail.to_string()
}

async fn poll_device_token(
    token_url: &str,
    client_id: &str,
    device_code: &str,
) -> Result<Option<(String, Option<String>)>> {
    let client = reqwest::Client::new();
    let response = client
        .post(token_url)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("grant_type", DEVICE_CODE_GRANT),
            ("device_code", device_code),
        ])
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if let Ok(res) = serde_json::from_str::<TokenResponse>(&body) {
        if let Some(token) = res.access_token {
            return Ok(Some((token, res.refresh_token)));
        }

        match res.error.as_deref() {
            Some("authorization_pending") | Some("slow_down") => return Ok(None),
            Some("expired_token") => {
                return Err(err("Sign-in timed out. Close the browser and try again."));
            }
            Some("access_denied") => {
                return Err(err("Sign-in was cancelled or denied in the browser."));
            }
            Some(e) => {
                let detail = res
                    .error_description
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| e.to_string());
                return Err(err(detail));
            }
            None => return Ok(None),
        }
    }

    if status.is_server_error() {
        return Err(err("Sign-in service is unavailable. Try again later."));
    }
    Ok(None)
}

fn oauth_state() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

fn oauth_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

fn build_ms_authorize_url(
    base: &str,
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
) -> String {
    format!(
        "{}?client_id={}&prompt=select_account&redirect_uri={}&response_type=code&scope={}&state={}",
        base,
        oauth_encode(client_id),
        oauth_encode(redirect_uri),
        oauth_encode(scope),
        oauth_encode(state),
    )
}

fn build_ely_authorize_url(
    base: &str,
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&prompt=select_account",
        base,
        oauth_encode(client_id),
        oauth_encode(redirect_uri),
        oauth_encode(scope),
        oauth_encode(state),
    )
}

async fn exchange_oauth_code(
    token_url: &str,
    client_id: &str,
    client_secret: Option<&str>,
    code: &str,
    redirect_uri: &str,
) -> Result<(String, Option<String>)> {
    let client = reqwest::Client::new();
    let mut form: Vec<(String, String)> = vec![
        ("grant_type".into(), AUTH_CODE_GRANT.into()),
        ("client_id".into(), client_id.to_string()),
        ("code".into(), code.to_string()),
        ("redirect_uri".into(), redirect_uri.to_string()),
    ];
    if let Some(secret) = client_secret {
        form.push(("client_secret".into(), secret.to_string()));
    }

    let response = client
        .post(token_url)
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if let Ok(res) = serde_json::from_str::<TokenResponse>(&body) {
        if let Some(token) = res.access_token {
            return Ok((token, res.refresh_token));
        }
        if let Some(e) = res.error.as_deref() {
            let detail = res
                .error_description
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| e.to_string());
            return Err(err(detail));
        }
    }

    if !status.is_success() {
        return Err(err("Could not complete sign-in. Try again."));
    }

    Err(err("Could not read the sign-in response. Try again."))
}

async fn ely_minecraft_profile(token: &str) -> Result<McProfile> {
    let client = reqwest::Client::new();
    client
        .get(ELY_PROFILE_URL)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/json")
        .send()
        .await?
        .json::<McProfile>()
        .await
        .map_err(|_| err("Could not load your Ely.by profile."))
}

pub async fn begin_device_login() -> Result<DeviceCodeSession> {
    let client_id = resolve_ms_client_id()?;
    request_device_code(DEVICE_CODE_URL, &client_id, SCOPE).await
}

pub async fn poll_device_login(session: &DeviceCodeSession) -> Result<Option<AuthAccount>> {
    let client_id = resolve_ms_client_id()?;
    if let Some((ms_access, refresh)) =
        poll_device_token(TOKEN_URL, &client_id, &session.device_code).await?
    {
        let account = complete_xbox_chain(&ms_access, refresh).await?;
        save_account(&account)?;
        return Ok(Some(account));
    }
    Ok(None)
}

pub async fn complete_device_login(session: DeviceCodeSession) -> Result<AuthAccount> {
    begin_login_flow();
    let interval = session.interval.max(5);
    let max_attempts = session.expires_in / interval + 1;
    tokio::time::sleep(std::time::Duration::from_secs(interval as u64)).await;
    for _ in 0..max_attempts {
        if is_login_cancelled() {
            return Err(err("Sign-in cancelled."));
        }
        if let Some(acc) = poll_device_login(&session).await? {
            return Ok(acc);
        }
        if is_login_cancelled() {
            return Err(err("Sign-in cancelled."));
        }
        tokio::time::sleep(std::time::Duration::from_secs(interval as u64)).await;
    }
    Err(err("Microsoft sign-in timed out. Try again."))
}

async fn complete_xbox_chain(ms_access: &str, refresh: Option<String>) -> Result<AuthAccount> {
    let xbl = xbox_live_auth(ms_access).await?;
    let xsts = xsts_auth(&xbl).await?;
    let uhs = xsts
        .DisplayClaims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .ok_or_else(|| err("XSTS missing user hash"))?;
    let mc = minecraft_auth(&uhs, &xsts.Token).await?;
    let profile = minecraft_profile(&mc).await?;

    Ok(AuthAccount {
        username: profile.name,
        uuid: format_uuid(&profile.id),
        access_token: mc,
        refresh_token: refresh,
        auth_type: "microsoft".into(),
        client_token: None,
    })
}

pub fn login_offline(username: String) -> Result<AuthAccount> {
    let username = username.trim();
    if username.is_empty() || username.len() > 16 {
        return Err(err("Username must be 1–16 characters."));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(err(
            "Username can only contain letters, numbers, and underscores.",
        ));
    }

    let account = AuthAccount {
        username: username.to_string(),
        uuid: offline_uuid(username),
        access_token: "offline".into(),
        refresh_token: None,
        auth_type: "offline".into(),
        client_token: None,
    };
    save_account(&account)?;
    Ok(account)
}

pub async fn login_elyby(
    username: String,
    password: String,
    totp: Option<String>,
) -> Result<AuthAccount> {
    let username = username.trim();
    if username.is_empty() {
        return Err(err("Username or email is required."));
    }

    let client_token = uuid::Uuid::new_v4().to_string();
    let password_value = match totp {
        Some(token) if !token.trim().is_empty() => format!("{}:{}", password, token.trim()),
        Some(_) | None => password,
    };

    let body = serde_json::json!({
        "agent": { "name": "Minecraft", "version": 1 },
        "username": username,
        "password": password_value,
        "clientToken": client_token,
        "requestUser": true
    });

    let client = reqwest::Client::new();
    let response = client
        .post(ELY_YGGDRASIL_AUTH_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        if let Ok(err_body) = serde_json::from_str::<ElyErrorResponse>(&text) {
            let msg = err_body
                .error_message
                .or(err_body.error)
                .unwrap_or_else(|| "Ely.by login failed.".into());
            if msg.contains("two factor") {
                return Err(err("Enter your Ely.by 2FA code and try again."));
            }
            return Err(err(msg));
        }
        return Err(err(
            "Ely.by sign-in failed. Check your username and password.",
        ));
    }

    let res: ElyAuthResponse = serde_json::from_str(&text)
        .map_err(|e| err(format!("Failed to parse Ely.by response: {e}")))?;

    let account = AuthAccount {
        username: res.selected_profile.name,
        uuid: format_uuid(&res.selected_profile.id),
        access_token: res.access_token,
        refresh_token: None,
        auth_type: "elyby".into(),
        client_token: Some(res.client_token),
    };
    save_account(&account)?;
    Ok(account)
}

pub async fn refresh_ely_account(account: &AuthAccount) -> Result<AuthAccount> {
    let client_token = account
        .client_token
        .as_ref()
        .ok_or_else(|| err("Missing Ely.by client token. Sign in again."))?;

    let body = serde_json::json!({
        "accessToken": account.access_token,
        "clientToken": client_token,
        "requestUser": true
    });

    let client = reqwest::Client::new();
    let response = client
        .post(ELY_REFRESH_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(err("Ely.by session expired. Sign in again."));
    }

    let res: ElyAuthResponse = serde_json::from_str(&text)
        .map_err(|e| err(format!("Failed to parse Ely.by refresh response: {e}")))?;

    let updated = AuthAccount {
        username: res.selected_profile.name,
        uuid: format_uuid(&res.selected_profile.id),
        access_token: res.access_token,
        refresh_token: None,
        auth_type: "elyby".into(),
        client_token: Some(res.client_token),
    };
    save_account(&updated)?;
    Ok(updated)
}

fn truncate_body(body: &str) -> String {
    let flat: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.len() > 400 {
        format!("{}…", &flat[..400])
    } else {
        flat
    }
}

fn xsts_error_message(code: u64) -> String {
    match code {
        2148916233 => {
            "This Microsoft account does not have an Xbox profile. Open xbox.com, sign in once, then try again.".into()
        }
        2148916235 => "Xbox Live is not available in this region.".into(),
        2148916236 => {
            "This account needs adult verification on xbox.com before you can sign in.".into()
        }
        2148916238 => {
            "This is a child Xbox account. Add it to a Microsoft family on xbox.com, then try again.".into()
        }
        2148916227 => {
            "Xbox Live rejected this sign-in. Confirm the Azure app includes XboxLive.signin.".into()
        }
        _ => format!("Xbox sign-in failed (code {code})."),
    }
}

fn auth_http_error(what: &str, status: reqwest::StatusCode, body: &str) -> crate::error::LauncherError {
    if let Ok(xbox) = serde_json::from_str::<XboxErrorBody>(body) {
        if let Some(code) = xbox.xerr {
            return err(xsts_error_message(code));
        }
        if let Some(msg) = xbox
            .error_message
            .or(xbox.message)
            .or(xbox.error)
            .filter(|s| !s.is_empty())
        {
            return err(msg);
        }
    }
    if status.as_u16() == 404 {
        return err("This Microsoft account does not own Minecraft Java Edition.");
    }
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return err(
            "Microsoft sign-in was rejected. Try again, or check Xbox Live permissions on the Azure app.",
        );
    }
    err(format!("{what} failed. Try again."))
}

async fn send_json<T: DeserializeOwned>(
    request: reqwest::RequestBuilder,
    what: &str,
) -> Result<T> {
    let response = request.send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        log::warn!("{what} HTTP {status}: {}", truncate_body(&body));
        return Err(auth_http_error(what, status, &body));
    }
    serde_json::from_str(&body).map_err(|e| {
        log::warn!("{what} JSON decode failed ({e}): {}", truncate_body(&body));
        err(format!("Could not read the {what} response. Try again."))
    })
}

async fn xbox_live_auth(ms_token: &str) -> Result<String> {
    let body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={}", ms_token),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let client = reqwest::Client::new();
    let res = send_json::<XboxAuthResponse>(
        client
            .post("https://user.auth.xboxlive.com/user/authenticate")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("x-xbl-contract-version", "1")
            .json(&body),
        "Xbox Live sign-in",
    )
    .await?;
    Ok(res.Token)
}

async fn xsts_auth(xbl_token: &str) -> Result<XstsResponse> {
    let body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token],
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let client = reqwest::Client::new();
    send_json(
        client
            .post("https://xsts.auth.xboxlive.com/xsts/authorize")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("x-xbl-contract-version", "1")
            .json(&body),
        "Xbox XSTS sign-in",
    )
    .await
}

async fn minecraft_auth(uhs: &str, xsts_token: &str) -> Result<String> {
    let identity = format!("XBL3.0 x={};{}", uhs, xsts_token);
    let body = serde_json::json!({ "identityToken": identity });
    let client = reqwest::Client::new();
    let res = send_json::<McAuthResponse>(
        client
            .post("https://api.minecraftservices.com/authentication/login_with_xbox")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body),
        "Minecraft sign-in",
    )
    .await?;
    Ok(res.access_token)
}

async fn minecraft_profile(token: &str) -> Result<McProfile> {
    let client = reqwest::Client::new();
    send_json(
        client
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json"),
        "Minecraft profile",
    )
    .await
}

fn offline_uuid(username: &str) -> String {
    let digest = md5::compute(format!("OfflinePlayer:{username}"));
    let mut bytes = digest.0;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn format_uuid(id: &str) -> String {
    if id.len() == 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &id[0..8],
            &id[8..12],
            &id[12..16],
            &id[16..20],
            &id[20..32]
        )
    } else {
        id.to_string()
    }
}

fn migrate_accounts_if_needed() -> Result<()> {
    let accounts_path = accounts_file();
    if accounts_path.exists() {
        return Ok(());
    }

    let old_path = account_file();
    if old_path.exists() {
        let data = std::fs::read_to_string(&old_path)?;
        let account: AuthAccount = serde_json::from_str(&data)?;
        let store = AccountsStore {
            accounts: vec![account.clone()],
            active_account_id: Some(account.uuid.clone()),
        };
        save_accounts_store(&store)?;
        std::fs::remove_file(&old_path)?;
    }

    Ok(())
}

fn load_accounts_store() -> Result<AccountsStore> {
    migrate_accounts_if_needed()?;
    let path = accounts_file();
    if !path.exists() {
        return Ok(AccountsStore {
            accounts: vec![],
            active_account_id: None,
        });
    }
    let data = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

fn save_accounts_store(store: &AccountsStore) -> Result<()> {
    if let Some(parent) = accounts_file().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(accounts_file(), serde_json::to_string_pretty(store)?)?;
    Ok(())
}

pub fn list_accounts_store() -> Result<AccountsStore> {
    load_accounts_store()
}

pub fn get_account() -> Result<Option<AuthAccount>> {
    let store = load_accounts_store()?;
    Ok(active_account_from_store(&store))
}

fn active_account_from_store(store: &AccountsStore) -> Option<AuthAccount> {
    let id = store.active_account_id.as_deref();
    store
        .accounts
        .iter()
        .find(|a| Some(a.uuid.as_str()) == id)
        .cloned()
}

pub fn save_account(account: &AuthAccount) -> Result<()> {
    let mut store = load_accounts_store()?;
    if let Some(pos) = store.accounts.iter().position(|a| a.uuid == account.uuid) {
        store.accounts[pos] = account.clone();
    } else {
        store.accounts.push(account.clone());
    }
    store.active_account_id = Some(account.uuid.clone());
    save_accounts_store(&store)?;
    Ok(())
}

pub fn switch_account(account_id: &str) -> Result<AuthAccount> {
    let mut store = load_accounts_store()?;
    let account = store
        .accounts
        .iter()
        .find(|a| a.uuid == account_id)
        .cloned()
        .ok_or_else(|| err("Account not found."))?;
    store.active_account_id = Some(account.uuid.clone());
    save_accounts_store(&store)?;
    Ok(account)
}

pub fn remove_account(account_id: &str) -> Result<()> {
    let mut store = load_accounts_store()?;
    let was_active = store.active_account_id.as_deref() == Some(account_id);
    store.accounts.retain(|a| a.uuid != account_id);
    if was_active {
        store.active_account_id = store.accounts.first().map(|a| a.uuid.clone());
    }
    save_accounts_store(&store)?;
    Ok(())
}

pub fn logout() -> Result<()> {
    let store = load_accounts_store()?;
    if let Some(id) = store.active_account_id.clone() {
        remove_account(&id)?;
    }
    Ok(())
}

pub async fn account_status() -> Result<Option<AuthAccount>> {
    let account = get_account()?;
    if let Some(acc) = account {
        if acc.auth_type == "elyby" {
            match refresh_ely_account(&acc).await {
                Ok(updated) => return Ok(Some(updated)),
                Err(_) => return Ok(Some(acc)),
            }
        }
        return Ok(Some(acc));
    }
    Ok(None)
}
