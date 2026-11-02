//! Opted-in metadata providers as playable books. Source references, not
//! expiring media URLs or credentials, are persisted in the existing tables.

use crate::core::app::error::{
    AccessErrorKind, InputErrorKind, LookupErrorKind, PluginErrorKind, Result, TingError,
};
use crate::core::storage::StorageService;
use crate::db::models::Library;
use crate::db::repository::BookRepository;
use crate::plugin::host_api::http;
use crate::plugin::sandbox::Permission;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ting_plugin_contract::capability::MetadataOperation;
use ting_plugin_contract::scraper::{
    ChapterListRequest, ChapterPage, MediaSourceDescriptor, MediaSourceType, SourceBookDetail,
    SourceRef,
};

use super::ScraperService;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BookSourceBinding {
    pub plugin_id: String,
}

#[derive(Debug, Serialize)]
pub struct AddedSourceBook {
    pub book_id: String,
    pub already_added: bool,
}

pub struct SourceAudio {
    pub descriptor: MediaSourceDescriptor,
    pub allowed_domains: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceChapterPage {
    pub total: Option<usize>,
    pub has_more: Option<bool>,
}

impl SourceAudio {
    pub async fn reader(
        &self,
        storage: &StorageService,
        range: Option<(u64, u64)>,
    ) -> Result<(Box<dyn tokio::io::AsyncRead + Send + Unpin>, u64)> {
        storage
            .get_http_reader(
                &self.descriptor.url,
                range,
                Some(&self.descriptor.headers),
                Some(&self.allowed_domains),
            )
            .await
    }
}

pub(crate) fn source_record_id(parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    format!("source-{:x}", digest.finalize())
}

fn source_error(message: impl Into<String>) -> TingError {
    TingError::validation(InputErrorKind::InvalidValue, "book_source", message.into())
}

async fn source_theme_color(
    plugin: &crate::plugin::types::PluginMetadata,
    cover_url: &str,
) -> Option<String> {
    let parsed = url::Url::parse(cover_url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        tracing::warn!(
            plugin_id = %plugin.id,
            cover_url,
            message_key = "book_source.theme_color.cover_scheme_unsupported",
            "Ignoring non-HTTP source cover URL"
        );
        return None;
    }
    let allowed_domains: Vec<String> = plugin
        .permissions
        .iter()
        .filter_map(|permission| {
            if let Permission::NetworkAccess { domain } = permission {
                Some(domain.clone())
            } else {
                None
            }
        })
        .collect();
    let options = serde_json::json!({
        "headers": {
            "User-Agent": "Mozilla/5.0 (Ting Reader)"
        },
        "timeout_ms": 10_000
    });
    let response = match http::request(cover_url, Some(&options), &allowed_domains).await {
        Ok(response) if (200..300).contains(&response.status) => response,
        Ok(response) => {
            tracing::warn!(
                plugin_id = %plugin.id,
                cover_url,
                status = response.status,
                message_key = "book_source.theme_color.cover_fetch_failed",
                "Source cover request returned a non-success status"
            );
            return None;
        }
        Err(error) => {
            tracing::warn!(
                plugin_id = %plugin.id,
                cover_url,
                error = %error,
                message_key = "book_source.theme_color.cover_fetch_failed",
                "Source cover request was rejected or failed"
            );
            return None;
        }
    };
    match crate::core::books::color::calculate_theme_color_from_bytes(&response.body).await {
        Ok(color) => color,
        Err(error) => {
            tracing::warn!(
                plugin_id = %plugin.id,
                cover_url,
                error = %error,
                message_key = "book_source.theme_color.calculate_failed",
                "Failed to calculate source book theme color"
            );
            None
        }
    }
}

impl ScraperService {
    /// Fetch and persist only the source chapter page needed by the detail
    /// view. Ordinary book libraries and legacy source plugins are untouched.
    pub async fn load_source_chapter_page(
        &self,
        books: &BookRepository,
        book: &crate::db::models::Book,
        library: &Library,
        offset: usize,
        limit: usize,
    ) -> Result<Option<SourceChapterPage>> {
        if library.library_type != "source" || limit == 0 {
            return Ok(None);
        }
        let binding: BookSourceBinding = library
            .scraper_config
            .as_deref()
            .ok_or_else(|| source_error("Missing book source binding"))
            .and_then(|json| {
                serde_json::from_str(json).map_err(|error| source_error(error.to_string()))
            })?;
        let source = self.book_source(&binding.plugin_id, true).await?;
        let plugin = self.plugin_manager.get_plugin(&source.id)?;
        let supports_listing = plugin.capabilities.iter().any(|capability| {
            matches!(
                capability,
                crate::plugin::types::PluginCapability::MetadataProvider(cap)
                    if cap.book_source
                        && cap.operations.contains(&MetadataOperation::ListChapters)
            )
        });
        if !supports_listing {
            return Ok(None);
        }

        let page_size = limit.clamp(1, 100);
        let target_page = offset / page_size + 1;
        let existing = books.source_chapter_count(&book.id).await?;
        let first_page = if existing < offset {
            existing / page_size + 1
        } else {
            target_page
        };
        let source_id = (!book.path.is_empty()).then(|| book.path.clone());
        let mut target = None;
        for page_number in first_page..=target_page {
            let request = ChapterListRequest {
                source: SourceRef {
                    id: source_id.clone(),
                    url: None,
                },
                page: page_number as u32,
                page_size: page_size as u32,
            };
            request.validate().map_err(source_error)?;
            let output = self
                .plugin_manager
                .invoke_metadata(
                    &source.id,
                    MetadataOperation::ListChapters,
                    serde_json::to_value(&request)
                        .map_err(|error| source_error(error.to_string()))?,
                )
                .await?;
            let page: ChapterPage =
                serde_json::from_value(output).map_err(|error| source_error(error.to_string()))?;
            page.validate().map_err(source_error)?;
            books.append_source_chapters(&book.id, &page.items).await?;
            if page_number == target_page {
                target = Some(page);
            }
        }
        let page = target.ok_or_else(|| source_error("Source chapter page was not loaded"))?;

        Ok(Some(SourceChapterPage {
            total: page.total.map(|total| total as usize),
            has_more: page.has_more,
        }))
    }

    async fn book_source(
        &self,
        source: &str,
        is_admin: bool,
    ) -> Result<crate::plugin::types::scraper::ScraperSourceInfo> {
        let mut sources: Vec<_> = self
            .get_sources()
            .await
            .into_iter()
            .filter(|info| info.enabled && info.book_source)
            .filter(|info| {
                info.id == source || info.id.rsplit_once('@').is_some_and(|(id, _)| id == source)
            })
            .collect();
        sources.sort_by(|left, right| {
            semver::Version::parse(&right.version)
                .ok()
                .cmp(&semver::Version::parse(&left.version).ok())
        });
        let source = sources.into_iter().next().ok_or_else(|| {
            TingError::not_found(
                LookupErrorKind::Plugin,
                "book_source",
                "Book source is not available".into(),
            )
        })?;
        if self.plugin_manager.get_plugin(&source.id)?.admin_only && !is_admin {
            return Err(TingError::permission(
                AccessErrorKind::ResourceDenied,
                "book_source",
                "This source is restricted to administrators".into(),
            ));
        }
        Ok(source)
    }

    pub async fn add_source_book(
        &self,
        books: &BookRepository,
        user_id: &str,
        is_admin: bool,
        source: &str,
        reference: SourceRef,
    ) -> Result<AddedSourceBook> {
        reference.validate().map_err(source_error)?;
        let source = self.book_source(source, is_admin).await?;
        let plugin = self.plugin_manager.get_plugin(&source.id)?;
        let output = self
            .plugin_manager
            .invoke_metadata(
                &source.id,
                MetadataOperation::Detail,
                serde_json::json!(reference),
            )
            .await?;
        let detail: SourceBookDetail =
            serde_json::from_value(output).map_err(|error| source_error(error.to_string()))?;
        detail.validate().map_err(source_error)?;
        if reference
            .id
            .as_ref()
            .is_some_and(|id| Some(id) != detail.metadata.id.as_ref())
        {
            return Err(source_error(
                "Book source detail ID does not match the requested book",
            ));
        }
        let cover_url = detail.metadata.cover_url.clone();
        let theme_color = cover_url
            .as_deref()
            .and_then(|cover_url| (!cover_url.trim().is_empty()).then_some(cover_url))
            .map(|cover_url| source_theme_color(&plugin, cover_url));
        let theme_color = match theme_color {
            Some(future) => future.await,
            None => None,
        };
        books
            .import_source_book(&plugin.id, &source.name, user_id, theme_color, detail)
            .await
    }

    /// Refresh every book already stored in a managed source library.
    pub async fn sync_source_library(
        &self,
        books: &BookRepository,
        library: &Library,
    ) -> Result<usize> {
        if library.library_type != "source" {
            return Err(source_error("Only managed source libraries can be synced"));
        }
        let binding: BookSourceBinding = library
            .scraper_config
            .as_deref()
            .ok_or_else(|| source_error("Missing book source binding"))
            .and_then(|json| {
                serde_json::from_str(json).map_err(|error| source_error(error.to_string()))
            })?;
        let source = self.book_source(&binding.plugin_id, true).await?;
        let plugin = self.plugin_manager.get_plugin(&source.id)?;
        let source_books = books.find_by_library(&library.id).await?;
        let mut updated = 0;

        for book in source_books {
            if book.path.trim().is_empty() {
                continue;
            }
            let reference = SourceRef {
                id: Some(book.path.clone()),
                url: None,
            };
            let output = self
                .plugin_manager
                .invoke_metadata(
                    &source.id,
                    MetadataOperation::Detail,
                    serde_json::to_value(&reference)
                        .map_err(|error| source_error(error.to_string()))?,
                )
                .await?;
            let detail: SourceBookDetail =
                serde_json::from_value(output).map_err(|error| source_error(error.to_string()))?;
            detail.validate().map_err(source_error)?;
            if detail.metadata.id.as_deref() != Some(book.path.as_str()) {
                return Err(source_error(format!(
                    "Book source detail ID does not match stored book '{}'",
                    book.path
                )));
            }

            let cover_url = detail.metadata.cover_url.clone();
            let theme_color = cover_url
                .as_deref()
                .filter(|url| !url.trim().is_empty())
                .map(|url| source_theme_color(&plugin, url));
            let theme_color = match theme_color {
                Some(future) => future.await,
                None => None,
            };
            books
                .update_source_book(&book.id, detail, theme_color)
                .await?;
            updated += 1;
        }

        Ok(updated)
    }

    pub async fn resolve_source_audio(
        &self,
        library: &Library,
        chapter_id: &str,
    ) -> Result<SourceAudio> {
        let binding: BookSourceBinding = library
            .scraper_config
            .as_deref()
            .ok_or_else(|| source_error("Missing book source binding"))
            .and_then(|json| {
                serde_json::from_str(json).map_err(|error| source_error(error.to_string()))
            })?;
        let source = self.book_source(&binding.plugin_id, true).await?;
        let plugin = self.plugin_manager.get_plugin(&source.id)?;
        let output = self
            .plugin_manager
            .invoke_metadata(
                &source.id,
                MetadataOperation::ResolveAudio,
                serde_json::json!({ "chapter_id": chapter_id }),
            )
            .await?;
        let descriptor: MediaSourceDescriptor =
            serde_json::from_value(output).map_err(|error| source_error(error.to_string()))?;
        descriptor.validate().map_err(source_error)?;
        // A playlist cannot be forwarded as if it were a single audio file:
        // its segments and encryption keys need their own authorized proxy.
        if matches!(descriptor.source_type, MediaSourceType::Hls)
            || descriptor.url.starts_with('/')
            || descriptor
                .mime
                .as_deref()
                .is_some_and(|mime| mime.to_ascii_lowercase().contains("mpegurl"))
            || url::Url::parse(&descriptor.url)
                .is_ok_and(|url| url.path().to_ascii_lowercase().ends_with(".m3u8"))
        {
            return Err(TingError::validation(
                InputErrorKind::UnsupportedValue, "book_source.audio",
                "Book sources currently require an absolute HTTP audio stream, not an HLS playlist or Host route".into(),
            ));
        }
        if let Some(expiry) = &descriptor.expires_at {
            let expiry = chrono::DateTime::parse_from_rfc3339(expiry)
                .map_err(|_| source_error("Invalid media expiry timestamp"))?;
            if expiry <= chrono::Utc::now() {
                return Err(TingError::plugin_execution(
                    PluginErrorKind::InvalidOutput,
                    "book_source.audio",
                    "Book source returned an expired playback URL".into(),
                ));
            }
        }
        let allowed_domains = plugin
            .permissions
            .into_iter()
            .filter_map(|permission| {
                if let Permission::NetworkAccess { domain } = permission {
                    Some(domain)
                } else {
                    None
                }
            })
            .collect();
        Ok(SourceAudio {
            descriptor,
            allowed_domains,
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/core/books/sources.rs"]
mod tests;
