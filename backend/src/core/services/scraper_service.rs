use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::core::error::{Result, TingError};
use crate::plugin::manager::{PluginManager, ScraperMethod};
use crate::plugin::scraper::{BookDetail, SearchResult};

/// Cache entry for scraper results
#[derive(Clone)]
struct CacheEntry<T> {
    data: T,
    expires_at: Instant,
}

/// Scraper service for coordinating scraper plugin operations
pub struct ScraperService {
    plugin_manager: Arc<PluginManager>,
    search_cache: Arc<RwLock<HashMap<String, CacheEntry<SearchResult>>>>,
    cache_ttl: Duration,
}

impl ScraperService {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self::with_cache_ttl(plugin_manager, Duration::from_secs(300))
    }

    pub fn with_cache_ttl(plugin_manager: Arc<PluginManager>, cache_ttl: Duration) -> Self {
        Self {
            plugin_manager,
            search_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl,
        }
    }

    pub async fn get_sources(&self) -> Vec<crate::api::models::ScraperSourceInfo> {
        use crate::plugin::types::{PluginState, PluginType};
        self.plugin_manager
            .find_plugins_by_type(PluginType::Scraper)
            .await
            .into_iter()
            .map(|info| {
                let capabilities = info
                    .scraper
                    .unwrap_or_else(crate::plugin::types::ScraperCapabilities::legacy_default);
                let search_fields = if capabilities.search_fields.is_empty() {
                    crate::plugin::types::ScraperCapabilities::legacy_default().search_fields
                } else {
                    capabilities.search_fields
                };
                let result_fields = if capabilities.result_fields.is_empty() {
                    crate::plugin::types::ScraperCapabilities::legacy_default().result_fields
                } else {
                    capabilities.result_fields
                };
                crate::api::models::ScraperSourceInfo {
                    id: info.id,
                    name: info.name,
                    description: Some(info.description),
                    version: info.version,
                    enabled: matches!(info.state, PluginState::Active),
                    auto_scrape: capabilities.auto_scrape,
                    search_fields,
                    result_fields,
                }
            })
            .collect()
    }

    async fn auto_source_ids(&self) -> HashSet<String> {
        self.get_sources()
            .await
            .into_iter()
            .filter(|source| source.enabled && source.auto_scrape)
            .map(|source| source.id)
            .collect()
    }

    async fn select_scraper(&self, source: Option<&str>) -> Result<String> {
        if let Some(source_id) = source {
            let sources = self.get_sources().await;
            if sources.iter().any(|s| s.id == source_id && s.enabled) {
                return Ok(source_id.to_string());
            }
            if let Some(s) = sources.iter().find(|s| s.name == source_id && s.enabled) {
                return Ok(s.id.clone());
            }
            return Err(TingError::PluginNotFound(format!(
                "Scraper source '{}' not found or not enabled",
                source_id
            )));
        }

        let sources = self.get_sources().await;
        let active_sources: Vec<_> = sources.into_iter().filter(|s| s.enabled).collect();
        if active_sources.is_empty() {
            return Err(TingError::PluginNotFound(
                "No active scraper plugins available".to_string(),
            ));
        }
        Ok(active_sources[0].id.clone())
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
        use serde_json::json;

        if query.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Search query cannot be empty".to_string(),
            ));
        }

        let clean_query = query
            .split('丨')
            .next()
            .unwrap_or(query)
            .split('|')
            .next()
            .unwrap_or(query)
            .trim();
        let clean_query = if clean_query.is_empty() {
            query
        } else {
            clean_query
        };

        let source_id = self.select_scraper(source).await?;
        let cache_key = format!(
            "{}:{}:{}:{}:{}:{}",
            source_id,
            clean_query,
            author.unwrap_or(""),
            narrator.unwrap_or(""),
            page,
            page_size
        );

        if let Some(cached) = self.get_cached_search(&cache_key) {
            tracing::debug!("命中搜索查询的缓存: {}", clean_query);
            return Ok(cached);
        }

        tracing::debug!(
            "未命中搜索查询的缓存: {}, 调用插件 {}",
            clean_query,
            source_id
        );
        let params =
            json!({ "query": clean_query, "author": author, "narrator": narrator, "page": page });

        let result = match self
            .plugin_manager
            .call_scraper(&source_id, ScraperMethod::Search, params.clone())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("刮削器插件 {} 失败: {}", source_id, e);
                if source.is_some() {
                    return Err(TingError::PluginExecutionError(format!(
                        "Scraper '{}' failed: {}",
                        source_id, e
                    )));
                }
                tracing::info!("尝试回退到另一个刮削器");
                let fallback_source = self.try_fallback_scraper(&source_id).await?;
                self.plugin_manager
                    .call_scraper(&fallback_source, ScraperMethod::Search, params)
                    .await
                    .map_err(|e| {
                        TingError::PluginExecutionError(format!(
                            "All scrapers failed. Last error: {}",
                            e
                        ))
                    })?
            }
        };

        let search_result: SearchResult = serde_json::from_value(result).map_err(|e| {
            TingError::DeserializationError(format!("Failed to parse search result: {}", e))
        })?;

        self.cache_search_result(&cache_key, search_result.clone());
        Ok(search_result)
    }

    pub async fn search_with_params(
        &self,
        search_params: &HashMap<String, String>,
        source: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<SearchResult> {
        use serde_json::json;

        let mut normalized_params = BTreeMap::new();
        for (key, value) in search_params {
            if !value.trim().is_empty() {
                normalized_params.insert(key.clone(), value.trim().to_string());
            }
        }

        if normalized_params.is_empty() {
            return Err(TingError::ValidationError(
                "At least one search parameter is required".to_string(),
            ));
        }

        let query = normalized_params
            .get("title")
            .or_else(|| normalized_params.get("query"))
            .cloned();
        let clean_query = query.map(|query| {
            let clean_query = query.split('|').next().unwrap_or(&query).trim().to_string();
            if clean_query.is_empty() {
                query
            } else {
                clean_query
            }
        });
        if let Some(clean_query) = &clean_query {
            normalized_params.insert("query".to_string(), clean_query.clone());
            normalized_params.insert("title".to_string(), clean_query.clone());
        }
        let author = normalized_params
            .get("author")
            .filter(|s| !s.trim().is_empty())
            .cloned();
        let narrator = normalized_params
            .get("narrator")
            .filter(|s| !s.trim().is_empty())
            .cloned();

        let source_id = self.select_scraper(source).await?;
        let params_key = serde_json::to_string(&normalized_params).unwrap_or_default();
        let cache_key = format!("{}:{}:{}:{}", source_id, params_key, page, page_size);

        if let Some(cached) = self.get_cached_search(&cache_key) {
            return Ok(cached);
        }

        let mut params = json!(normalized_params);
        if let Some(obj) = params.as_object_mut() {
            if let Some(clean_query) = clean_query {
                obj.insert("query".to_string(), json!(clean_query));
                obj.insert("title".to_string(), json!(clean_query));
            }
            if let Some(author) = author {
                obj.insert("author".to_string(), json!(author));
            }
            if let Some(narrator) = narrator {
                obj.insert("narrator".to_string(), json!(narrator));
            }
            obj.insert("page".to_string(), json!(page));
            obj.insert("page_size".to_string(), json!(page_size));
        }

        let result = match self
            .plugin_manager
            .call_scraper(&source_id, ScraperMethod::Search, params.clone())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("鍒墛鍣ㄦ彃浠?{} 澶辫触: {}", source_id, e);
                if source.is_some() {
                    return Err(TingError::PluginExecutionError(format!(
                        "Scraper '{}' failed: {}",
                        source_id, e
                    )));
                }
                let fallback_source = self.try_fallback_scraper(&source_id).await?;
                self.plugin_manager
                    .call_scraper(&fallback_source, ScraperMethod::Search, params)
                    .await
                    .map_err(|e| {
                        TingError::PluginExecutionError(format!(
                            "All scrapers failed. Last error: {}",
                            e
                        ))
                    })?
            }
        };

        let search_result: SearchResult = serde_json::from_value(result).map_err(|e| {
            TingError::DeserializationError(format!("Failed to parse search result: {}", e))
        })?;

        self.cache_search_result(&cache_key, search_result.clone());
        Ok(search_result)
    }

    async fn try_fallback_scraper(&self, failed_source: &str) -> Result<String> {
        let sources = self.get_sources().await;
        let fallback = sources
            .into_iter()
            .find(|s| s.enabled && s.id != failed_source);
        match fallback {
            Some(source) => {
                tracing::info!("使用备用刮削器: {}", source.id);
                Ok(source.id)
            }
            None => Err(TingError::PluginNotFound(
                "No fallback scraper available".to_string(),
            )),
        }
    }

    // ── Cache helpers ──

    fn get_cached_search(&self, key: &str) -> Option<SearchResult> {
        let cache = self.search_cache.read().ok()?;
        let entry = cache.get(key)?;
        if Instant::now() < entry.expires_at {
            Some(entry.data.clone())
        } else {
            None
        }
    }

    fn cache_search_result(&self, key: &str, result: SearchResult) {
        if let Ok(mut cache) = self.search_cache.write() {
            cache.insert(
                key.to_string(),
                CacheEntry {
                    data: result,
                    expires_at: Instant::now() + self.cache_ttl,
                },
            );
            if cache.len() > 100 {
                cache.retain(|_, entry| Instant::now() < entry.expires_at);
            }
        }
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.search_cache.write() {
            cache.clear();
        }
        tracing::info!("刮削器服务缓存已清除");
    }

    pub fn get_cache_stats(&self) -> usize {
        self.search_cache.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Scrape book metadata using the provided configuration strategy
    pub async fn scrape_book_metadata(
        &self,
        query: &str,
        config: &crate::db::models::ScraperConfig,
    ) -> Result<BookDetail> {
        let mut all_sources = HashSet::new();

        for s in &config.default_sources {
            all_sources.insert(s.clone());
        }
        if let Some(sources) = &config.author_sources {
            sources.iter().for_each(|s| {
                all_sources.insert(s.clone());
            });
        }
        if let Some(sources) = &config.narrator_sources {
            sources.iter().for_each(|s| {
                all_sources.insert(s.clone());
            });
        }
        if let Some(sources) = &config.cover_sources {
            sources.iter().for_each(|s| {
                all_sources.insert(s.clone());
            });
        }
        if let Some(sources) = &config.intro_sources {
            sources.iter().for_each(|s| {
                all_sources.insert(s.clone());
            });
        }
        if let Some(sources) = &config.tags_sources {
            sources.iter().for_each(|s| {
                all_sources.insert(s.clone());
            });
        }

        let auto_sources = self.auto_source_ids().await;
        all_sources.retain(|source| auto_sources.contains(source));

        if all_sources.is_empty() {
            if let Some(source) = auto_sources.iter().next() {
                all_sources.insert(source.clone());
            } else {
                return Err(TingError::NotFound(
                    "No active automatic scraper plugins available".to_string(),
                ));
            }
        }

        let mut source_results: HashMap<String, BookDetail> = HashMap::new();
        for source_id in all_sources {
            match self.search(query, None, None, Some(&source_id), 1, 1).await {
                Ok(search_res) => {
                    if let Some(item) = search_res.items.first() {
                        let detail = BookDetail {
                            id: item.id.clone(),
                            title: item.title.clone(),
                            author: item.author.clone(),
                            narrator: item.narrator.clone(),
                            cover_url: item.cover_url.clone(),
                            intro: item.intro.clone().unwrap_or_default(),
                            tags: item.tags.clone(),
                            chapter_count: item.chapter_count.unwrap_or(0),
                            duration: item.duration,
                            subtitle: item.subtitle.clone(),
                            published_year: item.published_year.clone(),
                            published_date: item.published_date.clone(),
                            publisher: item.publisher.clone(),
                            isbn: item.isbn.clone(),
                            asin: item.asin.clone(),
                            language: item.language.clone(),
                            explicit: item.explicit.unwrap_or(false),
                            abridged: item.abridged.unwrap_or(false),
                            genre: None,
                        };
                        source_results.insert(source_id, detail);
                    }
                }
                Err(e) => tracing::warn!("在 {} 上的搜索失败: {}", source_id, e),
            }
        }

        if source_results.is_empty() {
            return Err(TingError::NotFound(
                "No metadata found from any scraper".to_string(),
            ));
        }

        let mut final_detail = BookDetail {
            id: String::new(),
            title: query.to_string(),
            author: String::new(),
            narrator: None,
            cover_url: None,
            intro: String::new(),
            tags: vec![],
            chapter_count: 0,
            duration: None,
            subtitle: None,
            published_year: None,
            published_date: None,
            publisher: None,
            isbn: None,
            asin: None,
            language: None,
            explicit: false,
            abridged: false,
            genre: None,
        };

        macro_rules! get_effective_sources {
            ($specific:expr) => {{
                let mut sources = Vec::new();
                if let Some(s) = $specific {
                    sources.extend(s);
                }
                for ds in &config.default_sources {
                    if !sources.contains(&ds) {
                        sources.push(ds);
                    }
                }
                sources
            }};
        }

        // Title from default sources
        for source in &config.default_sources {
            if let Some(detail) = source_results.get(source) {
                if !detail.title.is_empty() {
                    final_detail.title = detail.title.clone();
                    break;
                }
            }
        }
        if final_detail.title == query {
            for detail in source_results.values() {
                if !detail.title.is_empty() {
                    final_detail.title = detail.title.clone();
                    break;
                }
            }
        }

        // Per-field merge from specific + default sources
        for source in get_effective_sources!(config.author_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if !detail.author.is_empty() {
                    final_detail.author = detail.author.clone();
                    break;
                }
            }
        }
        for source in get_effective_sources!(config.narrator_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if detail.narrator.is_some() {
                    final_detail.narrator = detail.narrator.clone();
                    break;
                }
            }
        }
        for source in get_effective_sources!(config.cover_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if detail.cover_url.is_some() {
                    final_detail.cover_url = detail.cover_url.clone();
                    break;
                }
            }
        }
        for source in get_effective_sources!(config.intro_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if !detail.intro.is_empty() {
                    final_detail.intro = detail.intro.clone();
                    break;
                }
            }
        }
        for source in get_effective_sources!(config.tags_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if !detail.tags.is_empty() {
                    final_detail.tags = detail.tags.clone();
                    break;
                }
            }
        }

        // Remaining fields from default sources
        for source in &config.default_sources {
            if let Some(detail) = source_results.get(source) {
                if final_detail.subtitle.is_none() {
                    final_detail.subtitle = detail.subtitle.clone();
                }
                if final_detail.published_year.is_none() {
                    final_detail.published_year = detail.published_year.clone();
                }
                if final_detail.published_date.is_none() {
                    final_detail.published_date = detail.published_date.clone();
                }
                if final_detail.publisher.is_none() {
                    final_detail.publisher = detail.publisher.clone();
                }
                if final_detail.isbn.is_none() {
                    final_detail.isbn = detail.isbn.clone();
                }
                if final_detail.asin.is_none() {
                    final_detail.asin = detail.asin.clone();
                }
                if final_detail.language.is_none() {
                    final_detail.language = detail.language.clone();
                }
                if detail.explicit {
                    final_detail.explicit = true;
                }
                if detail.abridged {
                    final_detail.abridged = true;
                }
            }
        }

        Ok(final_detail)
    }
}
