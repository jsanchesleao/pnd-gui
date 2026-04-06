use base64::{engine::general_purpose, Engine as _};

#[derive(serde::Serialize)]
struct DownloadResult {
    data: String,
    filename: Option<String>,
}

fn extract_filename_from_disposition(value: &str) -> Option<String> {
    // Prefer filename*=UTF-8''<percent-encoded> (RFC 5987) over plain filename=
    let mut plain: Option<String> = None;
    for part in value.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename*=") {
            // Strip optional encoding prefix, e.g. "UTF-8''"
            let name = if let Some(encoded) = rest.strip_prefix("UTF-8''") {
                percent_decode(encoded)
            } else {
                rest.trim_matches('"').to_string()
            };
            return Some(name);
        }
        if let Some(rest) = part.strip_prefix("filename=") {
            plain = Some(rest.trim_matches('"').to_string());
        }
    }
    plain
}

fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().and_then(|c| c.to_digit(16));
            let h2 = chars.next().and_then(|c| c.to_digit(16));
            if let (Some(h1), Some(h2)) = (h1, h2) {
                out.push(char::from(((h1 << 4) | h2) as u8));
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[tauri::command]
async fn download_url(url: String) -> Result<DownloadResult, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "HTTP {} {}",
            response.status().as_u16(),
            response.status().canonical_reason().unwrap_or("Unknown")
        ));
    }
    let filename = response
        .headers()
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .and_then(extract_filename_from_disposition);
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    Ok(DownloadResult {
        data: general_purpose::STANDARD.encode(&bytes),
        filename,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![download_url])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
