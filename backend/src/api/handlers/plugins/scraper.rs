//! Scraper discovery and search HTTP endpoints.

use crate::api::models::{ScraperSearchRequest, ScraperSourcesResponse, SearchResponse};
use crate::api::state::AppState;
use crate::auth::middleware::AuthUser;
use crate::core::app::error::Result;
use axum::{Json, extract::State, response::IntoResponse};

/// Handler for GET /api/v1/scraper/sources - Get list of scraper sources
pub async fn get_scraper_sources(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<impl IntoResponse> {
    let sources = state
        .scraper_service
        .get_sources()
        .await
        .into_iter()
        .filter(|source| {
            user.role == "admin"
                || state
                    .plugin_manager
                    .get_plugin(&source.id)
                    .is_ok_and(|plugin| !plugin.admin_only)
        })
        .collect();

    Ok(Json(ScraperSourcesResponse { sources }))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddSourceBookRequest {
    pub source: String,
    pub book: ting_plugin_contract::scraper::SourceRef,
}

pub async fn add_source_book(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<AddSourceBookRequest>,
) -> Result<impl IntoResponse> {
    let result = state
        .scraper_service
        .add_source_book(
            &state.book_repo,
            &user.id,
            user.role == "admin",
            &request.source,
            request.book,
        )
        .await?;
    Ok(Json(result))
}

/// Handler for POST /api/v1/scraper/search - Search for books using scraper
pub async fn scraper_search(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<ScraperSearchRequest>,
) -> Result<impl IntoResponse> {
    if let Some(source) = &request.source {
        let plugin = state.plugin_manager.get_plugin(source)?;
        if plugin.admin_only {
            crate::api::require_admin(&user)?;
        }
    }
    let page = request.page.unwrap_or(1);
    let page_size = request.page_size.unwrap_or(20);
    let mut search_params = request.search_params.unwrap_or_default();
    if let Some(query) = request.query {
        search_params.entry("title".to_string()).or_insert(query);
    }
    if let Some(author) = request.author {
        search_params.entry("author".to_string()).or_insert(author);
    }
    if let Some(narrator) = request.narrator {
        search_params
            .entry("narrator".to_string())
            .or_insert(narrator);
    }

    let result = state
        .scraper_service
        .search_with_params(&search_params, request.source.as_deref(), page, page_size)
        .await?;

    let mut response = serde_json::json!(SearchResponse {
        items: result.items,
        total: result.total,
        has_more: result.has_more,
        page: result.page,
        page_size: result.page_size,
    });
    if let Some(source) = &request.source {
        let plugin = state.plugin_manager.get_plugin(source)?;
        if plugin
            .capabilities
            .iter()
            .find_map(|cap| {
                if let crate::plugin::types::PluginCapability::MetadataProvider(cap) = cap {
                    Some(cap.book_source)
                } else {
                    None
                }
            })
            .unwrap_or(false)
        {
            let added = state
                .book_repo
                .source_books_for_user(&plugin.id, &user.id, user.role == "admin")
                .await?;
            if let Some(items) = response["items"].as_array_mut() {
                for item in items {
                    item["added_book_id"] = item["id"]
                        .as_str()
                        .and_then(|id| added.get(id))
                        .cloned()
                        .into();
                }
            }
        }
    }
    Ok(Json(response))
}
