use crate::error::{err, Result};
use crate::models::AuthAccount;
use crate::services::download::download_bytes;
use crate::services::paths::launcher_root;
use image::{imageops::FilterType, Rgba, RgbaImage};
use std::io::Cursor;
use std::path::PathBuf;

pub async fn ensure_avatar(account: &AuthAccount, size: u32) -> Result<String> {
    let path = avatar_cache_path(account, size);
    if path.is_file() {
        return Ok(path.to_string_lossy().to_string());
    }

    let png = match account.auth_type.as_str() {
        "elyby" => {
            let url = format!(
                "http://skinsystem.ely.by/skins/{}",
                urlencoding_encode(&account.username)
            );
            let skin_bytes = download_bytes(&url).await?;
            tokio::task::spawn_blocking(move || crop_minecraft_head(&skin_bytes, size))
                .await
                .map_err(|e| err(e.to_string()))??
        }
        _ => {
            let id = account.uuid.replace('-', "");
            let url = format!("https://mc-heads.net/avatar/{id}/{size}");
            download_bytes(&url).await?
        }
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &png)?;
    Ok(path.to_string_lossy().to_string())
}

fn avatar_cache_path(account: &AuthAccount, size: u32) -> PathBuf {
    launcher_root().join("avatar-cache").join(format!(
        "{}-{}-{}.png",
        account.auth_type, account.uuid, size
    ))
}

fn urlencoding_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
                out.push(char::from_digit((byte & 0x0f) as u32, 16).unwrap_or('0'));
            }
        }
    }
    out
}

fn crop_minecraft_head(skin_png: &[u8], size: u32) -> Result<Vec<u8>> {
    let img = image::load_from_memory(skin_png).map_err(|e| err(e.to_string()))?;
    let rgba = img.to_rgba8();

    let mut head = RgbaImage::new(8, 8);
    for y in 0..8 {
        for x in 0..8 {
            let base = rgba.get_pixel(8 + x, 8 + y);
            let mut pixel = *base;
            if rgba.width() >= 48 {
                let hat = rgba.get_pixel(40 + x, 8 + y);
                pixel = blend_pixel(pixel, *hat);
            }
            head.put_pixel(x, y, pixel);
        }
    }

    let resized = image::imageops::resize(&head, size, size, FilterType::Nearest);
    let mut buf = Vec::new();
    resized
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .map_err(|e| err(e.to_string()))?;
    Ok(buf)
}

fn blend_pixel(base: Rgba<u8>, overlay: Rgba<u8>) -> Rgba<u8> {
    if overlay[3] == 0 {
        return base;
    }
    let alpha = overlay[3] as f32 / 255.0;
    let inv = 1.0 - alpha;
    Rgba([
        ((base[0] as f32) * inv + (overlay[0] as f32) * alpha).round() as u8,
        ((base[1] as f32) * inv + (overlay[1] as f32) * alpha).round() as u8,
        ((base[2] as f32) * inv + (overlay[2] as f32) * alpha).round() as u8,
        255,
    ])
}
