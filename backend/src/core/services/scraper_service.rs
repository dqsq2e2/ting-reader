use std::collections::{HashMap, HashSet};
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
    detail_cache: Arc<RwLock<HashMap<String, CacheEntry<BookDetail>>>>,
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
            detail_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl,
        }
    }

    pub async fn get_sources(&self) -> Vec<crate::api::models::ScraperSourceInfo> {
        use crate::plugin::types::{PluginType, PluginState};
        self.plugin_manager
            .find_plugins_by_type(PluginType::Scraper)
            .await
            .into_iter()
            .map(|info| crate::api::models::ScraperSourceInfo {
                id: info.id,
                name: info.name,
                description: Some(info.description),
                version: info.version,
                enabled: matches!(info.state, PluginState::Active),
            })
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
            return Err(TingError::PluginNotFound(
                format!("Scraper source '{}' not found or not enabled", source_id)
            ));
        }

        let sources = self.get_sources().await;
        let active_sources: Vec<_> = sources.into_iter().filter(|s| s.enabled).collect();
        if active_sources.is_empty() {
            return Err(TingError::PluginNotFound("No active scraper plugins available".to_string()));
        }
        Ok(active_sources[0].id.clone())
    }

    async fn call_plugin_with_retry(&self, source_id: &str, method: ScraperMethod, params: serde_json::Value) -> Result<serde_json::Value> {
        let mut attempts = 0;
        let max_attempts = 3;
        let mut delay = Duration::from_secs(1);

        loop {
            attempts += 1;
            match tokio::time::timeout(Duration::from_secs(30), self.plugin_manager.call_scraper(&source_id.to_string(), method.clone(), params.clone()))
                .await
                .map_err(|_| TingError::PluginExecutionError(format!("Plugin {} timed out", source_id)))
                .and_then(|res| res)
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempts >= max_attempts {
                        return Err(TingError::PluginExecutionError(
                            format!("Plugin {} failed after {} attempts: {}", source_id, attempts, e)
                        ));
                    }
                    tracing::warn!("插件 {} 失败 (尝试 {}/{}): {}。将在 {:?} 后重试...", source_id, attempts, max_attempts, e, delay);
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        }
    }

    pub async fn search(
        &self, query: &str, author: Option<&str>, narrator: Option<&str>,
        source: Option<&str>, page: u32, page_size: u32,
    ) -> Result<SearchResult> {
        use serde_json::json;

        if query.trim().is_empty() {
            return Err(TingError::ValidationError("Search query cannot be empty".to_string()));
        }

        let clean_query = query.split('丨').next().unwrap_or(query)
            .split('|').next().unwrap_or(query).trim();
        let clean_query = if clean_query.is_empty() { query } else { clean_query };

        let source_id = self.select_scraper(source).await?;
        let cache_key = format!("{}:{}:{}:{}:{}:{}", source_id, clean_query, author.unwrap_or(""), narrator.unwrap_or(""), page, page_size);

        if let Some(cached) = self.get_cached_search(&cache_key) {
            tracing::debug!("命中搜索查询的缓存: {}", clean_query);
            return Ok(cached);
        }

        tracing::debug!("未命中搜索查询的缓存: {}, 调用插件 {}", clean_query, source_id);
        let params = json!({ "query": clean_query, "author": author, "narrator": narrator, "page": page });

        let result = match self.plugin_manager.call_scraper(&source_id, ScraperMethod::Search, params.clone()).await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("刮削器插件 {} 失败: {}", source_id, e);
                if source.is_some() {
                    return Err(TingError::PluginExecutionError(format!("Scraper '{}' failed: {}", source_id, e)));
                }
                tracing::info!("尝试回退到另一个刮削器");
                let fallback_source = self.try_fallback_scraper(&source_id).await?;
                self.plugin_manager.call_scraper(&fallback_source, ScraperMethod::Search, params).await
                    .map_err(|e| TingError::PluginExecutionError(format!("All scrapers failed. Last error: {}", e)))?
            }
        };

        let search_result: SearchResult = serde_json::from_value(result)
            .map_err(|e| TingError::DeserializationError(format!("Failed to parse search result: {}", e)))?;

        self.cache_search_result(&cache_key, search_result.clone());
        Ok(search_result)
    }

    pub async fn get_detail(&self, source: &str, book_id: &str) -> Result<BookDetail> {
        use serde_json::json;

        if source.trim().is_empty() { return Err(TingError::ValidationError("Source cannot be empty".to_string())); }
        if book_id.trim().is_empty() { return Err(TingError::ValidationError("Book ID cannot be empty".to_string())); }

        let cache_key = format!("{}:{}", source, book_id);
        if let Some(cached) = self.get_cached_detail(&cache_key) {
            tracing::debug!("命中书籍详情的缓存: {}:{}", source, book_id);
            return Ok(cached);
        }

        let source_id = self.select_scraper(Some(source)).await?;
        tracing::debug!("未命中书籍详情的缓存: {}:{}, 调用插件 {}", source, book_id, source_id);

        let params = json!({ "book_id": book_id });
        let result = self.call_plugin_with_retry(&source_id, ScraperMethod::GetDetail, params).await
            .map_err(|e| {
                tracing::error!("从 {} 获取书籍详情失败: {}", source_id, e);
                TingError::PluginExecutionError(format!("Failed to get book detail: {}", e))
            })?;

        let book_detail: BookDetail = serde_json::from_value(result)
            .map_err(|e| TingError::DeserializationError(format!("Failed to parse book detail: {}", e)))?;

        self.cache_detail(&cache_key, book_detail.clone());
        Ok(book_detail)
    }

    async fn try_fallback_scraper(&self, failed_source: &str) -> Result<String> {
        let sources = self.get_sources().await;
        let fallback = sources.into_iter().find(|s| s.enabled && s.id != failed_source);
        match fallback {
            Some(source) => {
                tracing::info!("使用备用刮削器: {}", source.id);
                Ok(source.id)
            }
            None => Err(TingError::PluginNotFound("No fallback scraper available".to_string())),
        }
    }

    // ── Cache helpers ──

    fn get_cached_search(&self, key: &str) -> Option<SearchResult> {
        let cache = self.search_cache.read().ok()?;
        let entry = cache.get(key)?;
        if Instant::now() < entry.expires_at { Some(entry.data.clone()) } else { None }
    }

    fn cache_search_result(&self, key: &str, result: SearchResult) {
        if let Ok(mut cache) = self.search_cache.write() {
            cache.insert(key.to_string(), CacheEntry { data: result, expires_at: Instant::now() + self.cache_ttl });
            if cache.len() > 100 { cache.retain(|_, entry| Instant::now() < entry.expires_at); }
        }
    }

    fn get_cached_detail(&self, key: &str) -> Option<BookDetail> {
        let cache = self.detail_cache.read().ok()?;
        let entry = cache.get(key)?;
        if Instant::now() < entry.expires_at { Some(entry.data.clone()) } else { None }
    }

    fn cache_detail(&self, key: &str, detail: BookDetail) {
        if let Ok(mut cache) = self.detail_cache.write() {
            cache.insert(key.to_string(), CacheEntry { data: detail, expires_at: Instant::now() + self.cache_ttl });
            if cache.len() > 100 { cache.retain(|_, entry| Instant::now() < entry.expires_at); }
        }
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.search_cache.write() { cache.clear(); }
        if let Ok(mut cache) = self.detail_cache.write() { cache.clear(); }
        tracing::info!("刮削器服务缓存已清除");
    }

    pub fn get_cache_stats(&self) -> (usize, usize) {
        let search_count = self.search_cache.read().map(|c| c.len()).unwrap_or(0);
        let detail_count = self.detail_cache.read().map(|c| c.len()).unwrap_or(0);
        (search_count, detail_count)
    }

    /// Scrape book metadata using the provided configuration strategy
    pub async fn scrape_book_metadata(
        &self, query: &str, config: &crate::db::models::ScraperConfig,
    ) -> Result<BookDetail> {
        let mut all_sources = HashSet::new();

        for s in &config.default_sources { all_sources.insert(s.clone()); }
        if let Some(sources) = &config.author_sources { sources.iter().for_each(|s| { all_sources.insert(s.clone()); }); }
        if let Some(sources) = &config.narrator_sources { sources.iter().for_each(|s| { all_sources.insert(s.clone()); }); }
        if let Some(sources) = &config.cover_sources { sources.iter().for_each(|s| { all_sources.insert(s.clone()); }); }
        if let Some(sources) = &config.intro_sources { sources.iter().for_each(|s| { all_sources.insert(s.clone()); }); }
        if let Some(sources) = &config.tags_sources { sources.iter().for_each(|s| { all_sources.insert(s.clone()); }); }

        if all_sources.is_empty() {
            match self.select_scraper(None).await {
                Ok(s) => { all_sources.insert(s); },
                Err(_) => return Err(TingError::NotFound("No active scraper plugins available".to_string())),
            }
        }

        let mut source_results: HashMap<String, BookDetail> = HashMap::new();
        for source_id in all_sources {
            match self.search(query, None, None, Some(&source_id), 1, 1).await {
                Ok(search_res) => {
                    if let Some(item) = search_res.items.first() {
                        let detail = BookDetail {
                            id: item.id.clone(), title: item.title.clone(), author: item.author.clone(),
                            narrator: item.narrator.clone(), cover_url: item.cover_url.clone(),
                            intro: item.intro.clone().unwrap_or_default(), tags: item.tags.clone(),
                            chapter_count: item.chapter_count.unwrap_or(0), duration: item.duration,
                            subtitle: item.subtitle.clone(), published_year: item.published_year.clone(),
                            published_date: item.published_date.clone(), publisher: item.publisher.clone(),
                            isbn: item.isbn.clone(), asin: item.asin.clone(), language: item.language.clone(),
                            explicit: item.explicit.unwrap_or(false), abridged: item.abridged.unwrap_or(false),
                            genre: None,
                        };
                        source_results.insert(source_id, detail);
                    }
                }
                Err(e) => tracing::warn!("在 {} 上的搜索失败: {}", source_id, e),
            }
        }

        if source_results.is_empty() {
            return Err(TingError::NotFound("No metadata found from any scraper".to_string()));
        }

        let mut final_detail = BookDetail {
            id: String::new(), title: query.to_string(), author: String::new(),
            narrator: None, cover_url: None, intro: String::new(), tags: vec![],
            chapter_count: 0, duration: None, subtitle: None, published_year: None,
            published_date: None, publisher: None, isbn: None, asin: None,
            language: None, explicit: false, abridged: false, genre: None,
        };

        macro_rules! get_effective_sources {
            ($specific:expr) => {{
                let mut sources = Vec::new();
                if let Some(s) = $specific { sources.extend(s); }
                for ds in &config.default_sources { if !sources.contains(&ds) { sources.push(ds); } }
                sources
            }};
        }

        // Title from default sources
        for source in &config.default_sources {
            if let Some(detail) = source_results.get(source) {
                if !detail.title.is_empty() { final_detail.title = detail.title.clone(); break; }
            }
        }
        if final_detail.title == query {
            for detail in source_results.values() {
                if !detail.title.is_empty() { final_detail.title = detail.title.clone(); break; }
            }
        }

        // Per-field merge from specific + default sources
        for source in get_effective_sources!(config.author_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if !detail.author.is_empty() { final_detail.author = detail.author.clone(); break; }
            }
        }
        for source in get_effective_sources!(config.narrator_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if detail.narrator.is_some() { final_detail.narrator = detail.narrator.clone(); break; }
            }
        }
        for source in get_effective_sources!(config.cover_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if detail.cover_url.is_some() { final_detail.cover_url = detail.cover_url.clone(); break; }
            }
        }
        for source in get_effective_sources!(config.intro_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if !detail.intro.is_empty() { final_detail.intro = detail.intro.clone(); break; }
            }
        }
        for source in get_effective_sources!(config.tags_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if !detail.tags.is_empty() { final_detail.tags = detail.tags.clone(); break; }
            }
        }

        // Remaining fields from default sources
        for source in &config.default_sources {
            if let Some(detail) = source_results.get(source) {
                if final_detail.subtitle.is_none() { final_detail.subtitle = detail.subtitle.clone(); }
                if final_detail.published_year.is_none() { final_detail.published_year = detail.published_year.clone(); }
                if final_detail.published_date.is_none() { final_detail.published_date = detail.published_date.clone(); }
                if final_detail.publisher.is_none() { final_detail.publisher = detail.publisher.clone(); }
                if final_detail.isbn.is_none() { final_detail.isbn = detail.isbn.clone(); }
                if final_detail.asin.is_none() { final_detail.asin = detail.asin.clone(); }
                if final_detail.language.is_none() { final_detail.language = detail.language.clone(); }
                if detail.explicit { final_detail.explicit = true; }
                if detail.abridged { final_detail.abridged = true; }
            }
        }

        Ok(final_detail)
    }
}
