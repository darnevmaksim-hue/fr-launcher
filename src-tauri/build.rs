fn escape_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn strip_quotes(value: &str) -> &str {
    let value = value.trim();
    if value.len() >= 2 {
        if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            return value[1..value.len() - 1].trim();
        }
    }
    value
}

fn load_env_value_from_file(path: &std::path::Path, key_name: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let content = content.strip_prefix('\u{feff}').unwrap_or(&content);
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim() == key_name {
                let value = strip_quotes(value);
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn load_env_value(key_name: &str, default: &str) -> String {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.parent().unwrap().join(".env"),
        manifest_dir.join(".env"),
        std::env::current_dir()
            .map(|dir| dir.join(".env"))
            .unwrap_or_default(),
    ];

    for path in candidates {
        if let Some(value) = load_env_value_from_file(&path, key_name) {
            return value;
        }
    }

    std::env::var(key_name)
        .ok()
        .map(|value| strip_quotes(&value).to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn obfuscate(value: &str) -> Vec<u8> {
    const MIX: u8 = 0x5C;
    value
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ MIX ^ (i as u8).wrapping_mul(13))
        .collect()
}

fn deobfuscate(data: &[u8]) -> String {
    const MIX: u8 = 0x5C;
    let bytes: Vec<u8> = data
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ MIX ^ (i as u8).wrapping_mul(13))
        .collect();
    String::from_utf8(bytes).unwrap_or_default()
}

fn write_embedded_secret(out_dir: &str, filename: &str, fn_name: &str, value: &str) {
    let encoded = obfuscate(value);
    assert_eq!(
        deobfuscate(&encoded),
        value,
        "secret obfuscation roundtrip failed for {fn_name}"
    );

    let dest = std::path::Path::new(out_dir).join(filename);
    let source = format!(
        r#"pub fn {fn_name}() -> String {{
    const MIX: u8 = 0x5C;
    const DATA: &[u8] = &[{data}];
    let bytes: Vec<u8> = DATA
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ MIX ^ (i as u8).wrapping_mul(13))
        .collect();
    String::from_utf8(bytes).unwrap_or_default()
}}
"#,
        data = escape_bytes(&encoded)
    );
    std::fs::write(&dest, source).unwrap_or_else(|e| panic!("failed to write {filename}: {e}"));
}

fn main() {
    println!("cargo:rerun-if-changed=../.env");
    println!("cargo:rerun-if-env-changed=CURSEFORGE_API_KEY");
    println!("cargo:rerun-if-env-changed=MICROSOFT_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=ELY_OAUTH_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=ELY_OAUTH_CLIENT_SECRET");

    let cf_key = load_env_value("CURSEFORGE_API_KEY", "");
    if cf_key.is_empty() {
        println!(
            "cargo:warning=CURSEFORGE_API_KEY is empty. CurseForge search will fail until it is set in .env or CI secrets."
        );
    }
    let ms_client_id = load_env_value("MICROSOFT_CLIENT_ID", "");
    let ely_client_id = load_env_value("ELY_OAUTH_CLIENT_ID", "horizonui-launcher");
    let ely_client_secret = load_env_value("ELY_OAUTH_CLIENT_SECRET", "");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");

    write_embedded_secret(
        &out_dir,
        "curseforge_key.rs",
        "embedded_curseforge_api_key",
        &cf_key,
    );
    write_embedded_secret(
        &out_dir,
        "ms_client_id.rs",
        "embedded_ms_client_id",
        &ms_client_id,
    );
    write_embedded_secret(
        &out_dir,
        "ely_client_id.rs",
        "embedded_ely_client_id",
        &ely_client_id,
    );
    write_embedded_secret(
        &out_dir,
        "ely_client_secret.rs",
        "embedded_ely_client_secret",
        &ely_client_secret,
    );

    tauri_build::build()
}
