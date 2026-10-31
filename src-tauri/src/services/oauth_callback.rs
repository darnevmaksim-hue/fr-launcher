use crate::error::{err, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

static OAUTH_CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn reset_oauth_cancel() {
    OAUTH_CANCELLED.store(false, Ordering::SeqCst);
}

pub fn cancel_oauth() {
    OAUTH_CANCELLED.store(true, Ordering::SeqCst);
}

pub fn is_oauth_cancelled() -> bool {
    OAUTH_CANCELLED.load(Ordering::SeqCst)
}

pub async fn listen_for_oauth_code(
    port: u16,
    expected_state: String,
    timeout_secs: u64,
) -> Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| err(format!("Could not start login listener: {e}")))?;

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        if is_oauth_cancelled() {
            return Err(err("Sign-in cancelled."));
        }

        if Instant::now() >= deadline {
            return Err(err("Sign-in timed out. Try again."));
        }

        let accept = tokio::time::timeout(Duration::from_secs(2), listener.accept()).await;

        let (mut stream, _) = match accept {
            Ok(Ok(pair)) => pair,
            Ok(Err(_)) | Err(_) => continue,
        };

        let mut buf = [0u8; 8192];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|e| err(format!("Login listener error: {e}")))?;
        let request = String::from_utf8_lossy(&buf[..n]);

        if let Some(path) = request.split_whitespace().nth(1) {
            let query = path.split('?').nth(1).unwrap_or("");
            let code = query_param(query, "code");
            let state = query_param(query, "state");

            if let Some(error) = query_param(query, "error") {
                let detail =
                    query_param(query, "error_description").unwrap_or_else(|| error.clone());
                return Err(err(format!("Sign-in failed: {detail}")));
            }

            if code.is_none() {
                continue;
            }

            if state.as_deref() != Some(expected_state.as_str()) {
                continue;
            }

            let body = "<html><body style=\"font-family:sans-serif;text-align:center;padding:48px;background:#111;color:#eee\"><h2>Signed in</h2><p>You can close this tab and return to HorizonUI Launcher.</p></body></html>";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes()).await;
            return Ok(code.unwrap());
        }
    }
}

fn query_param(query: &str, key: &str) -> Option<String> {
    for part in query.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            if k == key {
                return Some(pct_decode(v));
            }
        }
    }
    None
}

fn pct_decode(value: &str) -> String {
    let mut out = Vec::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&value[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn reserve_oauth_port() -> Result<u16> {
    const FIXED: u16 = 47852;
    if std::net::TcpListener::bind(("127.0.0.1", FIXED)).is_ok() {
        return Ok(FIXED);
    }
    pick_oauth_port()
}

pub fn oauth_redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}/")
}

pub fn pick_oauth_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| err(format!("Could not allocate login port: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| err(e.to_string()))?
        .port();
    Ok(port)
}
