//! Cover image proxy handler

use crate::core::error::{Result, TingError};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use crate::api::handlers::AppState;

/// Query parameters for cover image proxy
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyCoverQuery {
    pub path: String,
    pub library_id: Option<String>,
    pub book_id: Option<String>,
}

/// GET /api/proxy/cover - Proxy cover images (local files, external URLs, WebDAV)
pub async fn proxy_cover(
    State(_state): State<AppState>,
    Query(params): Query<ProxyCoverQuery>,
) -> Result<impl IntoResponse> {
    use axum::http::header;

    if params.path == "embedded://first-chapter" {
        return Err(TingError::NotFound("Embedded cover extraction not yet implemented".to_string()));
    }

    if params.path.starts_with("http") {
        let mut target_url = params.path.clone();
        let mut referer = "".to_string();

        if let Some(idx) = target_url.find("#referer=") {
            referer = target_url[idx + 9..].to_string();
            target_url = target_url[..idx].to_string();
        }

        tracing::info!("Proxying external image: {}, referer: {}", target_url, referer);

        let client = reqwest::Client::new();
        let mut req = client.get(&target_url)
            .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");

        if !referer.is_empty() {
            req = req.header(reqwest::header::REFERER, referer);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let bytes = resp.bytes().await.map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
                return Ok((StatusCode::OK, [
                    (header::CONTENT_TYPE, content_type),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                    (header::CACHE_CONTROL, "public, max-age=31536000".to_string()),
                    ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                ], bytes.to_vec()).into_response());
            }
            Ok(resp) => return Err(TingError::NotFound(format!("Failed to fetch external cover: HTTP {}", resp.status()))),
            Err(e) => return Err(TingError::NotFound(format!("Failed to fetch external cover: {}", e))),
        }
    }

    let normalized_path = params.path.replace('\\', "/");
    let image_path = std::path::Path::new(&normalized_path);

    let final_path = if image_path.exists() {
        image_path.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        let abs_path = cwd.join(image_path);
        if abs_path.exists() {
            abs_path
        } else if normalized_path.starts_with("./") {
            let stripped = cwd.join(&normalized_path[2..]);
            if stripped.exists() { stripped }
            else { return Err(TingError::NotFound(format!("Cover image not found: {}", params.path))); }
        } else {
            return Err(TingError::NotFound(format!("Cover image not found: {}", params.path)));
        }
    } else {
        return Err(TingError::NotFound(format!("Cover image not found: {}", params.path)));
    };

    let image_data = tokio::fs::read(&final_path).await?;
    let mime_type = mime_guess::from_path(&final_path).first_or_octet_stream().to_string();

    Ok((StatusCode::OK, [
        (header::CONTENT_TYPE, mime_type),
        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
        (header::CACHE_CONTROL, "public, max-age=31536000".to_string()),
        ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
    ], image_data).into_response())
}
