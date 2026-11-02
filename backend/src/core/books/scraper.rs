//! Book search, scraping and search-result caching.

use crate::core::app::error::{CodecErrorKind, InputErrorKind, LookupErrorKind};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::scrape_results::{
    collect_candidates, merge_source_results, prioritize_contributor_matches, rank_candidates,
    source_search_params,
};
use crate::core::app::error::{Result, TingError};
use crate::plugin::manager::PluginManager;
use crate::plugin::types::scraper::{BookDetail, BookItem, ScraperSourceInfo, SearchResult};
use ting_plugin_contract::capability::MetadataOperation;
use ting_plugin_contract::scraper::{
    ChapterCandidate, ScraperResult, SearchContext, SearchMode, SearchPage, SearchRequest,
};

const AGGREGATE_CANDIDATE_PAGE_SIZE: u32 = 20;
const MAX_SEARCH_CACHE_ENTRIES: usize = 100;

#[derive(Clone, PartialEq, Eq, Hash)]
struct SearchCacheKey {
    source_id: String,
    params: BTreeMap<String, String>,
    page: u32,
    page_size: u32,
}

struct SearchCacheEntry {
    result: SearchResult,
    expires_at: Instant,
}

/// Scraper service for coordinating scraper plugin operations
pub struct ScraperService {
    pub(super) plugin_manager: Arc<PluginManager>,
    search_cache: RwLock<HashMap<SearchCacheKey, SearchCacheEntry>>,
    cache_ttl: Duration,
}

impl ScraperService {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self::with_cache_ttl(plugin_manager, Duration::from_secs(300))
    }

    pub fn with_cache_ttl(plugin_manager: Arc<PluginManager>, cache_ttl: Duration) -> Self {
        Self {
            plugin_manager,
            search_cache: RwLock::new(HashMap::new()),
            cache_ttl,
        }
    }

    pub async fn get_sources(&self) -> Vec<ScraperSourceInfo> {
        use crate::plugin::types::PluginState;
        self.plugin_manager
            .find_plugins_by_capability_kind("metadata_provider")
            .await
            .into_iter()
            .filter_map(|info| {
                // Metadata dispatch and the derived search form both use the
                // first provider; discovery must describe that same provider.
                let provider = info.capabilities.iter().find_map(|capability| {
                    if let crate::plugin::types::PluginCapability::MetadataProvider(cap) =
                        capability
                    {
                        Some(cap)
                    } else {
                        None
                    }
                })?;
                let aggregate_auto_scrape = provider.aggregate_auto_scrape;
                let book_source = provider.book_source;
                info.scraper
                    .clone()
                    .map(|capabilities| (info, capabilities, aggregate_auto_scrape, book_source))
            })
            .map(|info| {
                let (info, capabilities, aggregate_auto_scrape, book_source) = info;
                let search_fields = capabilities.search_fields.clone();
                let result_fields = capabilities.result_fields.clone();
                let result_field_labels = capabilities.result_field_labels.clone();
                ScraperSourceInfo {
                    id: info.id,
                    name: info.name,
                    description: Some(info.description),
                    version: info.version,
                    enabled: matches!(info.state, PluginState::Active),
                    book_source,
                    auto_scrape: capabilities.auto_scrape,
                    aggregate_auto_scrape,
                    search_fields,
                    result_fields,
                    result_field_labels,
                }
            })
            .collect()
    }

    async fn select_scraper(&self, source: Option<&str>) -> Result<ScraperSourceInfo> {
        let mut sources = self.get_sources().await;
        if let Some(source_id) = source {
            let index = sources
                .iter()
                .position(|s| s.enabled && s.id == source_id)
                .or_else(|| {
                    sources
                        .iter()
                        .position(|s| s.enabled && s.name == source_id)
                });
            if let Some(index) = index {
                return Ok(sources.remove(index));
            }
            return Err(TingError::plugin_not_found(
                LookupErrorKind::Plugin,
                "select_scraper::plugin_not_found",
                format!("Scraper source '{}' not found or not enabled", source_id),
            ));
        }

        let mut active_sources: Vec<_> = sources.into_iter().filter(|s| s.enabled).collect();
        if active_sources.is_empty() {
            return Err(TingError::plugin_not_found(
                LookupErrorKind::Plugin,
                "select_scraper::plugin_not_found",
                "No active scraper plugins available".to_string(),
            ));
        }
        if let Some(index) = active_sources
            .iter()
            .position(|source| !source.aggregate_auto_scrape)
        {
            return Ok(active_sources.remove(index));
        }
        Ok(active_sources.remove(0))
    }

    fn search_request(
        title: Option<&str>,
        author: Option<&str>,
        narrator: Option<&str>,
        page: u32,
        page_size: u32,
        filters: serde_json::Map<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        Self::search_request_with_context(
            title,
            author,
            narrator,
            (page, page_size),
            filters,
            Vec::new(),
            None,
        )
    }

    fn search_request_with_context(
        title: Option<&str>,
        author: Option<&str>,
        narrator: Option<&str>,
        pagination: (u32, u32),
        filters: serde_json::Map<String, serde_json::Value>,
        chapter_candidates: Vec<ChapterCandidate>,
        context: Option<SearchContext>,
    ) -> Result<serde_json::Value> {
        let request = SearchRequest {
            title: title.map(str::to_owned),
            author: author.map(str::to_owned),
            narrator: narrator.map(str::to_owned),
            page: pagination.0,
            page_size: pagination.1,
            filters,
            chapter_candidates,
            context,
        };
        request.validate().map_err(|error| {
            TingError::validation(
                InputErrorKind::InvalidValue,
                "search_request_with_context::validation",
                error.into(),
            )
        })?;
        serde_json::to_value(request).map_err(|error| {
            let message = error.to_string();
            TingError::serialization(
                CodecErrorKind::InvalidData,
                "search_request_with_context::serialization",
                message,
            )
            .with_source(anyhow::anyhow!(error))
        })
    }

    fn parse_search_page(value: serde_json::Value) -> Result<SearchResult> {
        let page: SearchPage = serde_json::from_value(value).map_err(|error| {
            let message = format!("Invalid scraper search page: {error}");
            TingError::deserialization(
                CodecErrorKind::InvalidData,
                "parse_search_page::deserialization",
                message,
            )
            .with_source(anyhow::anyhow!(error))
        })?;
        page.validate().map_err(|error| {
            TingError::deserialization(
                CodecErrorKind::InvalidData,
                "parse_search_page::deserialization",
                error.into(),
            )
        })?;
        Ok(SearchResult {
            items: page
                .items
                .into_iter()
                .map(Self::book_item_from_result)
                .collect(),
            total: page.total,
            has_more: page.has_more,
            page: page.page,
            page_size: page.page_size,
        })
    }

    fn book_item_from_result(result: ScraperResult) -> BookItem {
        BookItem {
            id: result.id,
            source_url: result.source_url,
            title: result.title,
            author: result.author.unwrap_or_default(),
            narrator: result.narrator,
            cover_url: result.cover_url,
            intro: result.intro,
            subtitle: result.subtitle,
            publisher: result.publisher,
            language: result.language,
            genre: result.genre,
            published_year: result.published_year.map(|year| year.to_string()),
            published_date: result.published_date,
            isbn: result.isbn,
            asin: result.asin,
            explicit: result.explicit,
            abridged: result.abridged,
            tags: result.tags,
            duration: result.duration,
            score: result.score,
            chapter_title_template: result.chapter_title_template,
            chapter_titles: result.chapter_titles,
        }
    }

    fn take_chapter_candidates(
        scanner_context: &mut Option<serde_json::Value>,
    ) -> Vec<ChapterCandidate> {
        let chapters = scanner_context
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|object| {
                object.remove("directory");
                object.remove("chapters")
            });
        let Some(serde_json::Value::Array(chapters)) = chapters else {
            return Vec::new();
        };
        chapters
            .into_iter()
            .enumerate()
            .filter_map(|(index, chapter)| {
                let title = chapter
                    .get("title")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| chapter.get("filename").and_then(serde_json::Value::as_str))?
                    .trim();
                if title.is_empty() {
                    return None;
                }
                Some(ChapterCandidate {
                    id: index.to_string(),
                    title: title.to_string(),
                })
            })
            .collect()
    }

    pub async fn search(
        &self,
        query: &str,
        author: Option<&str>,
        narrator: Option<&str>,
        source: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<SearchResult> {
        if query.trim().is_empty() {
            return Err(TingError::validation(
                InputErrorKind::InvalidValue,
                "search::validation",
                "Search query cannot be empty".into(),
            ));
        }
        let mut params = HashMap::from([("title".into(), query.to_owned())]);
        if let Some(author) = author {
            params.insert("author".into(), author.to_owned());
        }
        if let Some(narrator) = narrator {
            params.insert("narrator".into(), narrator.to_owned());
        }
        self.search_with_params(&params, source, page, page_size)
            .await
    }

    pub async fn search_with_params(
        &self,
        search_params: &HashMap<String, String>,
        source: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<SearchResult> {
        let normalized_params: BTreeMap<String, String> = search_params
            .iter()
            .filter_map(|(key, value)| {
                let value = value.trim();
                (!value.is_empty()).then(|| (key.clone(), value.to_owned()))
            })
            .collect();
        if normalized_params.is_empty() {
            return Err(TingError::validation(
                InputErrorKind::MissingField,
                "search_with_params::validation",
                "At least one search parameter is required".into(),
            ));
        }
        let rank_result = |result: &mut SearchResult| {
            prioritize_contributor_matches(
                normalized_params
                    .get("title")
                    .map(String::as_str)
                    .unwrap_or_default(),
                normalized_params.get("author").map(String::as_str),
                normalized_params.get("narrator").map(String::as_str),
                result,
            );
        };
        let selected_source = self.select_scraper(source).await?;
        let source_id = &selected_source.id;
        let cache_key = SearchCacheKey {
            source_id: source_id.clone(),
            params: source_search_params(&normalized_params, &selected_source),
            page,
            page_size,
        };
        if let Some(mut result) = self.get_cached_search(&cache_key) {
            rank_result(&mut result);
            return Ok(result);
        }
        let mut result = if selected_source.aggregate_auto_scrape {
            self.search_aggregate_source(source_id, &cache_key.params, page, page_size)
                .await?
        } else {
            let params = Self::request_from_params(&cache_key.params, page, page_size)?;
            match self.invoke_search(source_id, params).await {
                Ok(result) => result,
                Err(error) if source.is_some() => return Err(error),
                Err(_) => {
                    let fallback = self.try_fallback_scraper(source_id).await?;
                    let params = Self::request_from_params(
                        &source_search_params(&normalized_params, &fallback),
                        page,
                        page_size,
                    )?;
                    // A fallback result must never occupy the primary source's cache.
                    return self
                        .invoke_search(&fallback.id, params)
                        .await
                        .map(|mut result| {
                            rank_result(&mut result);
                            result
                        })
                        .map_err(|error| {
                            error.with_plugin_execution_context("All scrapers failed. Last error")
                        });
                }
            }
        };
        self.cache_search_result(cache_key, result.clone());
        rank_result(&mut result);
        Ok(result)
    }

    fn request_from_params(
        params: &BTreeMap<String, String>,
        page: u32,
        page_size: u32,
    ) -> Result<serde_json::Value> {
        let filters = params
            .iter()
            .filter(|(key, _)| !matches!(key.as_str(), "title" | "author" | "narrator"))
            .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
            .collect();
        Self::search_request(
            params.get("title").map(String::as_str),
            params.get("author").map(String::as_str),
            params.get("narrator").map(String::as_str),
            page,
            page_size,
            filters,
        )
    }

    async fn invoke_search(
        &self,
        source: &String,
        request: serde_json::Value,
    ) -> Result<SearchResult> {
        let value = self
            .plugin_manager
            .invoke_metadata(source, MetadataOperation::Search, request)
            .await
            .map_err(|error| {
                error.with_plugin_execution_context(format!("Scraper '{source}' failed"))
            })?;
        Self::parse_search_page(value)
    }

    async fn try_fallback_scraper(&self, failed_source: &str) -> Result<ScraperSourceInfo> {
        let sources = self.get_sources().await;
        let fallback = sources
            .into_iter()
            .find(|s| s.enabled && s.id != failed_source && !s.aggregate_auto_scrape);
        match fallback {
            Some(source) => {
                tracing::debug!("Using fallback scraper: {}", source.id);
                Ok(source)
            }
            None => Err(TingError::plugin_not_found(
                LookupErrorKind::Plugin,
                "try_fallback_scraper::plugin_not_found",
                "No fallback scraper available".to_string(),
            )),
        }
    }

    async fn search_source_direct(
        &self,
        source: &ScraperSourceInfo,
        query: &str,
        author: Option<&str>,
        narrator: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<SearchResult> {
        let params = [
            ("title", Some(query)),
            ("author", author),
            ("narrator", narrator),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key.into(), value.to_owned())))
        .collect();
        let params =
            Self::request_from_params(&source_search_params(&params, source), page, page_size)?;

        self.invoke_search(&source.id, params)
            .await
            .map(|mut result| {
                prioritize_contributor_matches(query, author, narrator, &mut result);
                result
            })
            .map_err(|error| {
                error.with_plugin_execution_context("Failed while preparing aggregate candidates")
            })
    }

    fn get_cached_search(&self, key: &SearchCacheKey) -> Option<SearchResult> {
        let mut cache = self.search_cache.write().ok()?;
        let now = Instant::now();
        cache.retain(|_, entry| entry.expires_at > now);
        cache.get(key).map(|entry| entry.result.clone())
    }

    fn cache_search_result(&self, key: SearchCacheKey, result: SearchResult) {
        if self.cache_ttl.is_zero() {
            return;
        }
        let Some(expires_at) = Instant::now().checked_add(self.cache_ttl) else {
            return;
        };
        if let Ok(mut cache) = self.search_cache.write() {
            let now = Instant::now();
            cache.retain(|_, entry| entry.expires_at > now);
            if cache.len() >= MAX_SEARCH_CACHE_ENTRIES
                && !cache.contains_key(&key)
                && let Some(oldest) = cache
                    .iter()
                    .min_by_key(|(_, entry)| entry.expires_at)
                    .map(|(key, _)| key.clone())
            {
                cache.remove(&oldest);
            }
            cache.insert(key, SearchCacheEntry { result, expires_at });
        }
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.search_cache.write() {
            cache.clear();
        }
        tracing::info!("Scraper service cache cleared");
    }

    pub fn get_cache_stats(&self) -> usize {
        let Ok(mut cache) = self.search_cache.write() else {
            return 0;
        };
        let now = Instant::now();
        cache.retain(|_, entry| entry.expires_at > now);
        cache.len()
    }

    async fn clean_query_with_aggregate_sources(
        &self,
        query: &str,
        aggregate_sources: &[String],
        context: Option<&serde_json::Value>,
    ) -> String {
        let trimmed_query = query.trim();
        if trimmed_query.is_empty() || aggregate_sources.is_empty() {
            return query.to_string();
        }

        for aggregate_source in aggregate_sources {
            let Ok(cleanup_context) = Self::search_request_with_context(
                Some(trimmed_query),
                None,
                None,
                (1, 1),
                Default::default(),
                Vec::new(),
                Some(SearchContext {
                    mode: SearchMode::TitleCleanup,
                    candidates: Vec::new(),
                    merged_metadata: None,
                    scanner_context: context.cloned(),
                    scraper_query: None,
                }),
            ) else {
                break;
            };

            match self
                .plugin_manager
                .invoke_metadata(aggregate_source, MetadataOperation::Search, cleanup_context)
                .await
            {
                Ok(value) => {
                    if let Some(detail) = Self::detail_from_plugin_value(value) {
                        let cleaned = detail.title.trim();
                        if !cleaned.is_empty() {
                            if cleaned != trimmed_query {
                                tracing::info!(
                                    source_id = %aggregate_source,
                                    original_query = %trimmed_query,
                                    cleaned_query = %cleaned,
                                    "AI title cleanup changed scraper query"
                                );
                            }
                            return cleaned.to_string();
                        }
                    }
                }
                Err(e) => tracing::warn!(
                    source_id = %aggregate_source,
                    error = %e,
                    message_key = "scraper.title_cleanup.failed",
                    message_params = %serde_json::json!({
                        "source_id": aggregate_source,
                        "error": e.to_string(),
                    }),
                    "Aggregate scraper title cleanup failed"
                ),
            }
        }

        query.to_string()
    }

    fn source_id_scope_param(value: Option<&String>) -> Option<HashSet<String>> {
        value.map(|raw| {
            if let Ok(ids) = serde_json::from_str::<Vec<String>>(raw) {
                return ids
                    .into_iter()
                    .map(|id| id.trim().to_string())
                    .filter(|id| !id.is_empty())
                    .collect();
            }

            raw.split([',', ';', '\n'])
                .map(|id| id.trim().to_string())
                .filter(|id| !id.is_empty())
                .collect()
        })
    }

    async fn search_aggregate_source(
        &self,
        aggregate_source: &String,
        search_params: &BTreeMap<String, String>,
        page: u32,
        page_size: u32,
    ) -> Result<SearchResult> {
        use serde_json::json;

        let query = search_params
            .get("title")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TingError::validation(
                    InputErrorKind::MissingField,
                    "search_aggregate_source::validation",
                    "Aggregate scraper search requires a title".to_string(),
                )
            })?;
        let author = search_params
            .get("author")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());
        let narrator = search_params
            .get("narrator")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());

        let candidate_scope =
            Self::source_id_scope_param(search_params.get("candidate_sources")).unwrap_or_default();
        let candidate_sources: Vec<_> = self
            .get_sources()
            .await
            .into_iter()
            .filter(|source| {
                source.enabled
                    && source.auto_scrape
                    && source.id.as_str() != aggregate_source.as_str()
                    && !source.aggregate_auto_scrape
                    && candidate_scope.contains(&source.id)
            })
            .collect();

        let mut source_results: HashMap<String, BookDetail> = HashMap::new();
        let mut candidate_items = Vec::new();

        for source in &candidate_sources {
            let source_id = &source.id;
            match self
                .search_source_direct(
                    source,
                    query,
                    author,
                    narrator,
                    1,
                    AGGREGATE_CANDIDATE_PAGE_SIZE,
                )
                .await
            {
                Ok(search_res) => {
                    collect_candidates(
                        query,
                        source_id,
                        search_res,
                        &mut source_results,
                        &mut candidate_items,
                    )?;
                }
                Err(e) => tracing::warn!(
                    source_id = %source_id,
                    aggregate_source = %aggregate_source,
                    error = %e,
                    message_key = "scraper.search.failed",
                    message_params = %serde_json::json!({
                        "source_id": source_id,
                        "error": e.to_string(),
                    }),
                    "Scraper search failed while preparing aggregate search"
                ),
            }
        }

        rank_candidates(&mut candidate_items);
        candidate_items.truncate(20);

        let merge_config = crate::db::models::ScraperConfig {
            default_sources: candidate_sources
                .into_iter()
                .map(|source| source.id)
                .collect(),
            ..Default::default()
        };
        let merged_metadata = merge_source_results(query, &merge_config, &source_results);

        let aggregate_context = Self::search_request_with_context(
            Some(query),
            author,
            narrator,
            (page, page_size),
            Default::default(),
            Vec::new(),
            Some(SearchContext {
                mode: SearchMode::Aggregate,
                candidates: candidate_items,
                merged_metadata: Some(json!(merged_metadata)),
                scanner_context: Some(json!({
                "current_metadata": {
                    "title": query,
                    "author": author,
                    "narrator": narrator,
                }
                })),
                scraper_query: None,
            }),
        )?;

        let value = self
            .plugin_manager
            .invoke_metadata(
                aggregate_source,
                MetadataOperation::Search,
                aggregate_context,
            )
            .await
            .map_err(|e| {
                e.with_plugin_execution_context(format!(
                    "Aggregate scraper '{}' failed",
                    aggregate_source
                ))
            })?;

        Self::parse_search_page(value)
    }

    fn detail_from_plugin_value(value: serde_json::Value) -> Option<BookDetail> {
        Self::parse_search_page(value)
            .ok()?
            .items
            .first()
            .map(BookDetail::from)
    }

    /// Scrape book metadata using the provided configuration strategy
    pub async fn scrape_book_metadata(
        &self,
        query: &str,
        config: &crate::db::models::ScraperConfig,
    ) -> Result<BookDetail> {
        self.scrape_book_metadata_with_context(query, config, None)
            .await
    }

    /// Scrape book metadata and optionally pass scanner context to aggregate providers.
    pub async fn scrape_book_metadata_with_context(
        &self,
        query: &str,
        config: &crate::db::models::ScraperConfig,
        context: Option<serde_json::Value>,
    ) -> Result<BookDetail> {
        if !config.has_scraper_sources() {
            return Err(TingError::not_found(
                LookupErrorKind::Plugin,
                "scrape_book_metadata_with_context::not_found",
                "No scraper plugins selected for this library".into(),
            ));
        }
        let sources = self.get_sources().await;
        let auto_sources: HashSet<_> = sources
            .iter()
            .filter(|source| source.enabled && source.auto_scrape)
            .map(|source| source.id.as_str())
            .collect();
        let aggregate_sources: HashSet<_> = sources
            .iter()
            .filter(|source| source.aggregate_auto_scrape)
            .map(|source| source.id.as_str())
            .collect();
        let mut seen = HashSet::new();
        let all_sources: Vec<String> = config
            .scraper_sources()
            .filter(|source| auto_sources.contains(*source) && seen.insert(*source))
            .map(str::to_owned)
            .collect();

        if all_sources.is_empty() {
            return Err(TingError::not_found(
                LookupErrorKind::Plugin,
                "scrape_book_metadata_with_context::not_found",
                "No selected automatic scraper plugins are active".into(),
            ));
        }

        let aggregate_sources_to_run: Vec<String> = all_sources
            .iter()
            .filter(|source| aggregate_sources.contains(source.as_str()))
            .cloned()
            .collect();
        let scrape_sources: Vec<String> = all_sources
            .iter()
            .filter(|source| !aggregate_sources.contains(source.as_str()))
            .cloned()
            .collect();
        let directory_name = context
            .as_ref()
            .and_then(|context| context.get("directory_name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(query);
        let name = super::text_cleaner::BookName::parse(directory_name);
        let metadata_value = |key| {
            context
                .as_ref()
                .and_then(|context| context.get("current_metadata"))
                .and_then(|metadata| metadata.get(key))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        };
        let author = metadata_value("author").or(name.author).map(str::to_owned);
        let narrator = metadata_value("narrator")
            .or(name.narrator)
            .map(str::to_owned);
        let scraper_query = self
            .clean_query_with_aggregate_sources(
                super::text_cleaner::book_title_from_name(query),
                &aggregate_sources_to_run,
                context.as_ref(),
            )
            .await;

        let mut source_results: HashMap<String, BookDetail> = HashMap::new();
        let mut candidate_items = Vec::new();
        for source_id in scrape_sources {
            match self
                .search(
                    &scraper_query,
                    author.as_deref(),
                    narrator.as_deref(),
                    Some(&source_id),
                    1,
                    AGGREGATE_CANDIDATE_PAGE_SIZE,
                )
                .await
            {
                Ok(mut search_res) => {
                    prioritize_contributor_matches(
                        &scraper_query,
                        author.as_deref(),
                        narrator.as_deref(),
                        &mut search_res,
                    );
                    collect_candidates(
                        &scraper_query,
                        &source_id,
                        search_res,
                        &mut source_results,
                        &mut candidate_items,
                    )?;
                }
                Err(e) => tracing::warn!(
                    source_id = %source_id,
                    error = %e,
                    message_key = "scraper.search.failed",
                    message_params = %serde_json::json!({
                        "source_id": source_id,
                        "error": e.to_string(),
                    }),
                    "Scraper search failed"
                ),
            }
        }

        rank_candidates(&mut candidate_items);

        if source_results.is_empty() && aggregate_sources_to_run.is_empty() {
            return Err(TingError::not_found(
                LookupErrorKind::Resource,
                "scrape_book_metadata_with_context::not_found",
                "No metadata found from any scraper".to_string(),
            ));
        }

        let final_detail = merge_source_results(query, config, &source_results);

        if !aggregate_sources_to_run.is_empty() {
            let mut scanner_context = context;
            let chapter_candidates = Self::take_chapter_candidates(&mut scanner_context);
            let batches: Vec<&[ChapterCandidate]> = if chapter_candidates.is_empty() {
                vec![&[]]
            } else {
                chapter_candidates.chunks(500).collect()
            };

            for aggregate_source in aggregate_sources_to_run {
                let mut selected = None;
                let mut cleaned_titles = Vec::new();
                for batch in &batches {
                    let aggregate_request = Self::search_request_with_context(
                        Some(query),
                        author.as_deref(),
                        narrator.as_deref(),
                        (1, 1),
                        Default::default(),
                        batch.to_vec(),
                        Some(SearchContext {
                            mode: SearchMode::Aggregate,
                            candidates: candidate_items.clone(),
                            merged_metadata: Some(serde_json::json!(final_detail)),
                            scanner_context: scanner_context.clone(),
                            scraper_query: Some(scraper_query.clone()),
                        }),
                    )?;
                    match self
                        .plugin_manager
                        .invoke_metadata(
                            &aggregate_source,
                            MetadataOperation::Search,
                            aggregate_request,
                        )
                        .await
                    {
                        Ok(value) => {
                            if let Some(mut detail) = Self::detail_from_plugin_value(value) {
                                if !batch.is_empty() {
                                    if detail.chapter_titles.len() != batch.len() {
                                        detail.chapter_titles =
                                            batch.iter().map(|c| c.title.clone()).collect();
                                    }
                                    cleaned_titles.extend(detail.chapter_titles.iter().cloned());
                                }
                                if selected.is_none() {
                                    selected = Some(detail);
                                }
                            }
                        }
                        Err(error) => {
                            tracing::warn!(
                                source_id = %aggregate_source,
                                %error,
                                "Aggregate scraper failed for a chapter batch"
                            );
                            cleaned_titles.extend(batch.iter().map(|c| c.title.clone()));
                        }
                    }
                }
                if let Some(mut detail) = selected {
                    if !chapter_candidates.is_empty() {
                        detail.chapter_titles = cleaned_titles;
                    }
                    return Ok(detail);
                }
            }
        }

        if source_results.is_empty() {
            return Err(TingError::not_found(
                LookupErrorKind::Resource,
                "scrape_book_metadata_with_context::not_found",
                "No metadata found from any scraper".to_string(),
            ));
        }

        Ok(final_detail)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/core/books/scraper.rs"]
mod tests;
