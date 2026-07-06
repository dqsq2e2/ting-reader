//! Cover image proxy handler

use crate::api::handlers::AppState;
use crate::core::error::{Result, TingError};
use crate::db::repository::Repository;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::path::{Component, Path, PathBuf};

/// Query parameters for cover image proxy
#[derive(Debug, serde::Deserialize)]
pub struct ProxyCoverQuery {
    pub path: String,
    pub library_id: Option<String>,
    pub book_id: Option<String>,
}

/// GET /api/proxy/cover - Proxy cover images (local files, external URLs, WebDAV)
pub async fn proxy_cover(
    State(state): State<AppState>,
    Query(params): Query<ProxyCoverQuery>,
) -> Result<impl IntoResponse> {
    use axum::http::header;

    if params.path == "embedded://first-chapter" {
        return Err(TingError::NotFound(
            "Embedded cover extraction not yet implemented".to_string(),
        ));
    }

    if params.path.starts_with("http") {
        let mut target_url = params.path.clone();
        let mut referer = "".to_string();

        if let Some(idx) = target_url.find("#referer=") {
            referer = target_url[idx + 9..].to_string();
            target_url = target_url[..idx].to_string();
        }

        tracing::info!(
            "Proxying external image: {}, referer: {}",
            target_url,
            referer
        );

        let client = reqwest::Client::new();
        let mut req = client.get(&target_url)
            .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");

        if !referer.is_empty() {
            req = req.header(reqwest::header::REFERER, referer);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let content_type = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let bytes = resp.bytes().await.map_err(|e| {
                    TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e))
                })?;
                return Ok((
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, content_type),
                        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                        (
                            header::CACHE_CONTROL,
                            "public, max-age=31536000".to_string(),
                        ),
                        (
                            "Cross-Origin-Resource-Policy".parse().unwrap(),
                            "cross-origin".to_string(),
                        ),
                    ],
                    bytes.to_vec(),
                )
                    .into_response());
            }
            Ok(resp) => {
                return Err(TingError::NotFound(format!(
                    "Failed to fetch external cover: HTTP {}",
                    resp.status()
                )))
            }
            Err(e) => {
                return Err(TingError::NotFound(format!(
                    "Failed to fetch external cover: {}",
                    e
                )))
            }
        }
    }

    let final_path = resolve_cover_path(&state, &params).await?;

    let image_data = tokio::fs::read(&final_path).await?;
    let mime_type = mime_guess::from_path(&final_path)
        .first_or_octet_stream()
        .to_string();

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime_type),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
            (
                header::CACHE_CONTROL,
                "no-cache, max-age=0, must-revalidate".to_string(),
            ),
            (
                "Cross-Origin-Resource-Policy".parse().unwrap(),
                "cross-origin".to_string(),
            ),
        ],
        image_data,
    )
        .into_response())
}

async fn resolve_cover_path(state: &AppState, params: &ProxyCoverQuery) -> Result<PathBuf> {
    let normalized_path = params.path.replace('\\', "/");
    let image_path = Path::new(&normalized_path);

    if image_path.exists() {
        return Ok(image_path.to_path_buf());
    }

    if !image_path.is_absolute() {
        let relative = normalize_cover_relative_path(&normalized_path)?;

        if let Some(book_id) = params.book_id.as_deref() {
            if let Some(book) = state.book_repo.find_by_id(book_id).await? {
                let candidate = Path::new(&book.path).join(&relative);
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }

        if let Some(library_id) = params.library_id.as_deref() {
            if let Some(library) = state.library_repo.find_by_id(library_id).await? {
                let root_path = library.root_path.trim();
                if !root_path.is_empty() {
                    let candidate = Path::new(root_path).join(&relative);
                    if candidate.exists() {
                        return Ok(candidate);
                    }
                }
            }
        }

        if let Ok(cwd) = std::env::current_dir() {
            let candidate = cwd.join(&relative);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err(TingError::NotFound(format!(
        "Cover image not found: {}",
        params.path
    )))
}

fn normalize_cover_relative_path(value: &str) -> Result<PathBuf> {
    let raw = Path::new(value.trim());
    if raw.is_absolute() {
        return Err(TingError::SecurityViolation(
            "Cover image path must be relative".to_string(),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in raw.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(TingError::SecurityViolation(
                    "Cover image path cannot escape its base directory".to_string(),
                ));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(TingError::InvalidRequest(
            "Cover image path is required".to_string(),
        ));
    }

    Ok(normalized)
}
