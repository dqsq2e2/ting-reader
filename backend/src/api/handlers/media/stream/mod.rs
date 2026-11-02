use crate::core::app::error::{
    AccessErrorKind, AuthenticationErrorKind, InputErrorKind, LookupErrorKind,
};
mod decrypt;
mod hls;
mod hls_serve;
mod hls_session;
pub(crate) mod preload;
mod source;
mod strm;
mod transcode;

use crate::api::state::AppState;
use crate::auth::middleware::AuthUser;
use crate::core::app::error::{Result, TingError};
use crate::core::security::signing::{
    constant_time_eq, sign_media_stream_request, signature_has_expired,
};
use crate::db::models::{Chapter, Library};
use crate::db::repository::Repository;
use axum::extract::{Path, Query, State};
pub(crate) use decrypt::create_decrypted_stream;
pub use hls::handle_hls_request;
pub use hls_serve::{
    close_hls_stream, get_hls_file, get_hls_playlist, get_hls_segment, seek_hls_stream,
    touch_hls_stream,
};
pub use hls_session::HlsSessionManager;

/// Query parameters for stream chapter
#[derive(Debug, serde::Deserialize)]
pub struct StreamQuery {
    pub token: Option<String>,
    pub transcode: Option<String>,
    pub seek: Option<String>,
    pub download: Option<String>,
    pub preload: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SignedStreamQuery {
    pub expires: Option<i64>,
    pub user: Option<String>,
    pub signature: Option<String>,
    pub transcode: Option<String>,
    pub seek: Option<String>,
    pub download: Option<String>,
}

/// Resolved and authorized media request shared by source handlers.
struct StreamContext {
    state: AppState,
    chapter: Chapter,
    book: crate::db::models::Book,
    library: Library,
    params: StreamQuery,
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
    user: Option<AuthUser>,
    source_audio: Option<crate::core::books::sources::SourceAudio>,
}

fn stream_mime_type_from_path(path: &str) -> String {
    crate::core::audio::AudioFormat::content_type(path, None)
}

fn is_download_query(params: &StreamQuery) -> bool {
    let Some(value) = params.download.as_deref() else {
        return false;
    };
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

pub(crate) async fn get_remote_media_reader(
    state: &AppState,
    library: &Library,
    path: &str,
    range: Option<(u64, u64)>,
) -> Result<(Box<dyn tokio::io::AsyncRead + Send + Unpin>, u64)> {
    if library.library_type == "source" {
        return state
            .scraper_service
            .resolve_source_audio(library, path)
            .await?
            .reader(&state.storage_service, range)
            .await;
    }
    if library.library_type == "webdav" {
        return state
            .storage_service
            .get_webdav_reader(library, path, range, state.encryption_key.as_ref())
            .await
            .map_err(|e| {
                let message = format!("Remote WebDAV media not found: {}", e);
                TingError::not_found(
                    LookupErrorKind::File,
                    "get_remote_media_reader::not_found",
                    message,
                )
                .with_source(anyhow::anyhow!(e))
            });
    }

    if library.library_type == "rss" || path.starts_with("http://") || path.starts_with("https://")
    {
        return state
            .storage_service
            .get_http_reader(path, range, None, None)
            .await
            .map_err(|e| {
                let message = format!("Remote media not found: {}", e);
                TingError::not_found(
                    LookupErrorKind::File,
                    "get_remote_media_reader::not_found",
                    message,
                )
                .with_source(anyhow::anyhow!(e))
            });
    }

    Err(TingError::validation(
        InputErrorKind::UnsupportedValue,
        "get_remote_media_reader::validation",
        format!("Unsupported remote library type '{}'", library.library_type),
    ))
}

/// Handler for GET /api/stream/:chapterId - Stream chapter audio
pub async fn stream_chapter(
    State(state): State<AppState>,
    Path(chapter_id): Path<String>,
    Query(params): Query<StreamQuery>,
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
    user: Option<AuthUser>,
) -> Result<axum::response::Response> {
    let chapter = state
        .chapter_repo
        .find_by_id(&chapter_id)
        .await?
        .ok_or_else(|| {
            TingError::not_found(
                LookupErrorKind::Chapter,
                "stream_chapter::not_found",
                format!("Chapter {} not found", chapter_id),
            )
        })?;

    let book = state
        .book_repo
        .find_by_id(&chapter.book_id)
        .await?
        .ok_or_else(|| {
            TingError::not_found(
                LookupErrorKind::Chapter,
                "stream_chapter::not_found",
                format!("Book {} not found", chapter.book_id),
            )
        })?;

    ensure_user_can_stream_book(&state, user.as_ref(), &book.id).await?;

    let library = state
        .library_repo
        .find_by_id(&book.library_id)
        .await?
        .ok_or_else(|| {
            TingError::not_found(
                LookupErrorKind::Book,
                "stream_chapter::not_found",
                format!("Library {} not found", book.library_id),
            )
        })?;

    let ext = std::path::Path::new(&chapter.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_download_request = is_download_query(&params);

    // STRM playback resolves redirects before returning the final source URL.
    if library.library_type != "source" && ext == "strm" {
        if !is_download_request {
            preload::cancel_auto_preload(&state, user.as_ref()).await;
        }
        return strm::handle_strm_stream(state, &chapter_id, chapter, book, library, &params).await;
    }
    if params.transcode.is_some() && !is_download_request {
        preload::cancel_auto_preload(&state, user.as_ref()).await;
    }
    // Handle HLS Transcoding Request
    if let Some(format) = &params.transcode
        && format == "hls"
    {
        tracing::info!("Requested HLS transcoding: {}", chapter.path);
        return handle_hls_request(
            state,
            chapter,
            book,
            library,
            ext == "strm",
            params.seek.clone(),
        )
        .await;
    }

    if params.transcode.is_some() {
        return transcode::transcode_stream(&state, &chapter, &library, &params).await;
    }

    let source_audio = if library.library_type == "source" {
        Some(
            state
                .scraper_service
                .resolve_source_audio(&library, &chapter.path)
                .await?,
        )
    } else {
        None
    };
    source::stream_source(StreamContext {
        state,
        chapter,
        book,
        library,
        params,
        method,
        headers,
        user,
        source_audio,
    })
    .await
}

/// Handler for GET /api/v1/public/media/:chapterId - Stream signed public chapter audio.
pub async fn stream_signed_chapter(
    State(state): State<AppState>,
    Path(chapter_id): Path<String>,
    Query(params): Query<SignedStreamQuery>,
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response> {
    validate_signed_stream_query(&state, &chapter_id, &params)?;

    let user_id = params.user.as_deref().unwrap_or_default();
    let signed_user = state.user_repo.find_by_id(user_id).await?.ok_or_else(|| {
        TingError::permission(
            AccessErrorKind::ResourceDenied,
            "stream_signed_chapter::permission",
            "Signed media route user not found".to_string(),
        )
    })?;
    let auth_user = AuthUser {
        user_id: signed_user.id.clone(),
        id: signed_user.id,
        username: signed_user.username,
        role: signed_user.role,
    };

    stream_chapter(
        State(state),
        Path(chapter_id),
        Query(StreamQuery {
            token: None,
            transcode: params.transcode,
            seek: params.seek,
            download: params.download,
            preload: None,
        }),
        method,
        headers,
        Some(auth_user),
    )
    .await
}

fn validate_signed_stream_query(
    state: &AppState,
    chapter_id: &str,
    params: &SignedStreamQuery,
) -> Result<()> {
    let expires = params.expires.ok_or_else(|| {
        TingError::permission(
            AccessErrorKind::ResourceDenied,
            "validate_signed_stream_query::permission",
            "Missing signed media expiry".to_string(),
        )
    })?;
    if signature_has_expired(expires) {
        return Err(TingError::permission(
            AccessErrorKind::ResourceDenied,
            "validate_signed_stream_query::permission",
            "Signed media URL has expired".to_string(),
        ));
    }

    let user_id = params
        .user
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            TingError::permission(
                AccessErrorKind::ResourceDenied,
                "validate_signed_stream_query::permission",
                "Missing signed media user".to_string(),
            )
        })?;
    let signature = params
        .signature
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            TingError::permission(
                AccessErrorKind::ResourceDenied,
                "validate_signed_stream_query::permission",
                "Missing signed media signature".to_string(),
            )
        })?;
    let download = signed_download_flag(params.download.as_deref());
    let expected = sign_media_stream_request(
        state.encryption_key.as_ref(),
        chapter_id,
        expires,
        user_id,
        params.transcode.as_deref(),
        params.seek.as_deref(),
        download,
    );

    if !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
        return Err(TingError::permission(
            AccessErrorKind::ResourceDenied,
            "validate_signed_stream_query::permission",
            "Invalid signed media signature".to_string(),
        ));
    }

    Ok(())
}

async fn ensure_user_can_stream_book(
    state: &AppState,
    user: Option<&AuthUser>,
    book_id: &str,
) -> Result<()> {
    let user = user.ok_or_else(|| {
        TingError::authentication(
            AuthenticationErrorKind::MissingCredentials,
            "ensure_user_can_stream_book::authentication",
            "用户未认证".to_string(),
        )
    })?;
    let is_admin = user.role == "admin";
    let can_access = state
        .book_repo
        .check_access(book_id, &user.id, is_admin)
        .await?;
    if can_access {
        Ok(())
    } else {
        Err(TingError::permission(
            AccessErrorKind::ResourceDenied,
            "ensure_user_can_stream_book::permission",
            format!("User cannot access book {}", book_id),
        ))
    }
}

fn signed_download_flag(value: Option<&str>) -> bool {
    value
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}
