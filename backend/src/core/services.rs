//! Business logic services
//!
//! This module implements the Application Layer services that coordinate between
//! the REST API Layer and the Infrastructure Layer (database, plugins, etc.).

use crate::core::error::{Result, TingError};
use crate::db::models::Book;
use crate::db::repository::{BookRepository, Repository};
use crate::api::models::{CreateBookRequest, UpdateBookRequest};
use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;

/// Book service for managing book business logic
pub struct BookService {
    book_repo: Arc<BookRepository>,
}

impl BookService {
    /// Create a new BookService
    pub fn new(book_repo: Arc<BookRepository>) -> Self {
        Self { book_repo }
    }
    
    /// Get all books
    pub async fn get_all_books(&self) -> Result<Vec<Book>> {
        self.book_repo.find_all().await
    }
    
    /// Get books by library ID
    pub async fn get_books_by_library(&self, library_id: &str) -> Result<Vec<Book>> {
        // Validate library_id is not empty
        if library_id.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Library ID cannot be empty".to_string()
            ));
        }
        
        self.book_repo.find_by_library(library_id).await
    }
    
    /// Get a book by ID
    pub async fn get_book_by_id(&self, id: &str) -> Result<Option<Book>> {
        // Validate ID is not empty
        if id.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Book ID cannot be empty".to_string()
            ));
        }
        
        self.book_repo.find_by_id(id).await
    }
    
    /// Get a book by hash
    pub async fn get_book_by_hash(&self, hash: &str) -> Result<Option<Book>> {
        // Validate hash is not empty
        if hash.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Book hash cannot be empty".to_string()
            ));
        }
        
        self.book_repo.find_by_hash(hash).await
    }
    
    /// Create a new book
    pub async fn create_book(&self, request: CreateBookRequest) -> Result<Book> {
        // Validate required fields
        self.validate_create_request(&request)?;
        
        // Check if book with same hash already exists
        if let Some(existing) = self.book_repo.find_by_hash(&request.hash).await? {
            return Err(TingError::ValidationError(
                format!("Book with hash {} already exists with ID {}", request.hash, existing.id)
            ));
        }
        
        // Generate new book ID
        let book_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        
        // Create book entity
        let book = Book {
            id: book_id.clone(),
            library_id: request.library_id,
            title: request.title,
            author: request.author,
            narrator: request.narrator,
            cover_url: request.cover_url,
            theme_color: request.theme_color,
            description: request.description,
            skip_intro: request.skip_intro,
            skip_outro: request.skip_outro,
            path: request.path,
            hash: request.hash,
            tags: request.tags,
            created_at: now,
            manual_corrected: 0,
            match_pattern: None,
            chapter_regex: None,
        };
        
        // Save to database
        self.book_repo.create(&book).await?;
        
        Ok(book)
    }
    
    /// Update an existing book
    pub async fn update_book(&self, id: &str, request: UpdateBookRequest) -> Result<Book> {
        // Validate ID
        if id.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Book ID cannot be empty".to_string()
            ));
        }
        
        // Get existing book
        let mut book = self.book_repo.find_by_id(id).await?
            .ok_or_else(|| TingError::NotFound(format!("Book with ID {} not found", id)))?;
        
        // Update fields if provided
        if let Some(library_id) = request.library_id {
            if library_id.trim().is_empty() {
                return Err(TingError::ValidationError(
                    "Library ID cannot be empty".to_string()
                ));
            }
            book.library_id = library_id;
        }
        
        if let Some(title) = request.title {
            book.title = Some(title);
        }
        
        if let Some(author) = request.author {
            book.author = Some(author);
        }
        
        if let Some(narrator) = request.narrator {
            book.narrator = Some(narrator);
        }
        
        if let Some(cover_url) = request.cover_url {
            // Only update if changed
            let should_update = match &book.cover_url {
                Some(current) => current != &cover_url,
                None => true,
            };

            if should_update {
                book.cover_url = Some(cover_url.clone());
                // Recalculate theme color if not explicitly provided in request
                if request.theme_color.is_none() {
                    use crate::core::color::calculate_theme_color;
                    tracing::info!("Recalculating theme color for book {} from cover {}", book.id, cover_url);
                    
                    match calculate_theme_color(&cover_url).await {
                        Ok(Some(color)) => {
                            tracing::info!("Updated theme color for book {}: {}", book.id, color);
                            book.theme_color = Some(color);
                        }
                        Ok(None) => {
                            tracing::warn!("Could not extract theme color from cover {}", cover_url);
                            book.theme_color = None;
                        }
                        Err(e) => {
                            tracing::error!("Failed to calculate theme color: {}", e);
                            book.theme_color = None;
                        }
                    }
                }
            }
        }
        
        if let Some(theme_color) = request.theme_color {
            book.theme_color = Some(theme_color);
        }
        
        if let Some(description) = request.description {
            book.description = Some(description);
        }
        
        if let Some(skip_intro) = request.skip_intro {
            if skip_intro < 0 {
                return Err(TingError::ValidationError(
                    "skip_intro cannot be negative".to_string()
                ));
            }
            book.skip_intro = skip_intro;
        }
        
        if let Some(skip_outro) = request.skip_outro {
            if skip_outro < 0 {
                return Err(TingError::ValidationError(
                    "skip_outro cannot be negative".to_string()
                ));
            }
            book.skip_outro = skip_outro;
        }
        
        if let Some(path) = request.path {
            if path.trim().is_empty() {
                return Err(TingError::ValidationError(
                    "Book path cannot be empty".to_string()
                ));
            }
            book.path = path;
        }
        
        if let Some(hash) = request.hash {
            if hash.trim().is_empty() {
                return Err(TingError::ValidationError(
                    "Book hash cannot be empty".to_string()
                ));
            }
            // Check if another book with this hash exists
            if let Some(existing) = self.book_repo.find_by_hash(&hash).await? {
                if existing.id != id {
                    return Err(TingError::ValidationError(
                        format!("Another book with hash {} already exists with ID {}", hash, existing.id)
                    ));
                }
            }
            book.hash = hash;
        }
        
        if let Some(tags) = request.tags {
            book.tags = Some(tags);
        }
        
        // Save updated book
        self.book_repo.update(&book).await?;
        
        Ok(book)
    }
    
    /// Delete a book by ID
    pub async fn delete_book(&self, id: &str) -> Result<()> {
        // Validate ID
        if id.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Book ID cannot be empty".to_string()
            ));
        }
        
        // Check if book exists
        let book = self.book_repo.find_by_id(id).await?
            .ok_or_else(|| TingError::NotFound(format!("Book with ID {} not found", id)))?;
        
        // Delete the book
        self.book_repo.delete(&book.id).await?;
        
        Ok(())
    }
    
    /// Validate create book request
    fn validate_create_request(&self, request: &CreateBookRequest) -> Result<()> {
        // Validate library_id
        if request.library_id.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Library ID is required and cannot be empty".to_string()
            ));
        }
        
        // Validate path
        if request.path.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Book path is required and cannot be empty".to_string()
            ));
        }
        
        // Validate hash
        if request.hash.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Book hash is required and cannot be empty".to_string()
            ));
        }
        
        // Validate skip_intro and skip_outro are non-negative
        if request.skip_intro < 0 {
            return Err(TingError::ValidationError(
                "skip_intro cannot be negative".to_string()
            ));
        }
        
        if request.skip_outro < 0 {
            return Err(TingError::ValidationError(
                "skip_outro cannot be negative".to_string()
            ));
        }
        
        Ok(())
    }
}

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Cache entry for scraper results
#[derive(Clone)]
struct CacheEntry<T> {
    data: T,
    expires_at: Instant,
}

/// Scraper service for coordinating scraper plugin operations
pub struct ScraperService {
    plugin_manager: Arc<crate::plugin::manager::PluginManager>,
    /// Search result cache (query_key -> cached result)
    search_cache: Arc<RwLock<HashMap<String, CacheEntry<crate::plugin::scraper::SearchResult>>>>,
    /// Book detail cache (source:book_id -> cached detail)
    detail_cache: Arc<RwLock<HashMap<String, CacheEntry<crate::plugin::scraper::BookDetail>>>>,
    /// Cache TTL (time to live)
    cache_ttl: Duration,
}

impl ScraperService {
    /// Create a new ScraperService with default cache TTL (5 minutes)
    pub fn new(plugin_manager: Arc<crate::plugin::manager::PluginManager>) -> Self {
        Self::with_cache_ttl(plugin_manager, Duration::from_secs(300))
    }
    
    /// Create a new ScraperService with custom cache TTL
    pub fn with_cache_ttl(
        plugin_manager: Arc<crate::plugin::manager::PluginManager>,
        cache_ttl: Duration,
    ) -> Self {
        Self {
            plugin_manager,
            search_cache: Arc::new(RwLock::new(HashMap::new())),
            detail_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl,
        }
    }
    
    /// Get list of available scraper sources
    pub async fn get_sources(&self) -> Vec<crate::api::models::ScraperSourceInfo> {
        use crate::plugin::types::PluginType;
        use crate::plugin::types::PluginState;
        
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
    
    /// Select the best available scraper plugin
    /// Priority: 1) Specified source, 2) First active scraper
    async fn select_scraper(&self, source: Option<&str>) -> Result<String> {
        if let Some(source_id) = source {
            // Verify the specified source exists and is active
            let sources = self.get_sources().await;
            
            // Try exact match on ID first
            if sources.iter().any(|s| s.id == source_id && s.enabled) {
                return Ok(source_id.to_string());
            }
            
            // Try match on name
            if let Some(s) = sources.iter().find(|s| s.name == source_id && s.enabled) {
                return Ok(s.id.clone());
            }

            return Err(TingError::PluginNotFound(
                format!("Scraper source '{}' not found or not enabled", source_id)
            ));
        }
        
        // Get first active scraper
        let sources = self.get_sources().await;
        let active_sources: Vec<_> = sources.into_iter()
            .filter(|s| s.enabled)
            .collect();
        
        if active_sources.is_empty() {
            return Err(TingError::PluginNotFound(
                "No active scraper plugins available".to_string()
            ));
        }
        
        Ok(active_sources[0].id.clone())
    }
    
    /// Search for books using a specific scraper or the best available scraper
    pub async fn search(
        &self,
        query: &str,
        source: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<crate::plugin::scraper::SearchResult> {
        use crate::plugin::manager::ScraperMethod;
        use serde_json::json;
        
        // Validate input
        if query.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Search query cannot be empty".to_string()
            ));
        }

        // Clean query: remove suffix after first "丨" or "|" or "-" if present
        // This handles cases like "乱世书丨头陀渊工作室丨历史穿越..."
        let clean_query = query
            .split('丨')
            .next()
            .unwrap_or(query)
            .split('|')
            .next()
            .unwrap_or(query)
            .trim();
        
        let clean_query = if clean_query.is_empty() { query } else { clean_query };
        
        // Select scraper plugin
        let source_id = self.select_scraper(source).await?;
        
        // Generate cache key (use clean_query)
        let cache_key = format!("{}:{}:{}:{}", source_id, clean_query, page, page_size);
        
        // Check cache first
        if let Some(cached) = self.get_cached_search(&cache_key) {
            tracing::debug!("Cache hit for search query: {}", clean_query);
            return Ok(cached);
        }
        
        tracing::debug!("Cache miss for search query: {}, calling plugin {}", clean_query, source_id);
        
        // Call scraper plugin with error handling and fallback
        let params = json!({
            "query": clean_query,
            "page": page,
        });
        
        let result = match self.plugin_manager
            .call_scraper(&source_id, ScraperMethod::Search, params.clone())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Scraper plugin {} failed: {}", source_id, e);
                
                // If a specific source was requested, don't try fallback
                if source.is_some() {
                    return Err(TingError::PluginExecutionError(
                        format!("Scraper '{}' failed: {}", source_id, e)
                    ));
                }
                
                // Try fallback to another scraper
                tracing::info!("Attempting fallback to another scraper");
                let fallback_source = self.try_fallback_scraper(&source_id).await?;
                
                self.plugin_manager
                    .call_scraper(&fallback_source, ScraperMethod::Search, params)
                    .await
                    .map_err(|e| TingError::PluginExecutionError(
                        format!("All scrapers failed. Last error: {}", e)
                    ))?
            }
        };
        
        // Parse result
        let search_result: crate::plugin::scraper::SearchResult = 
            serde_json::from_value(result)
                .map_err(|e| TingError::DeserializationError(
                    format!("Failed to parse search result: {}", e)
                ))?;
        
        // Cache the result
        self.cache_search_result(&cache_key, search_result.clone());
        
        Ok(search_result)
    }
    
    /// Get detailed information about a book from a specific scraper
    pub async fn get_detail(
        &self,
        source: &str,
        book_id: &str,
    ) -> Result<crate::plugin::scraper::BookDetail> {
        use crate::plugin::manager::ScraperMethod;
        use serde_json::json;
        
        // Validate input
        if source.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Source cannot be empty".to_string()
            ));
        }
        if book_id.trim().is_empty() {
            return Err(TingError::ValidationError(
                "Book ID cannot be empty".to_string()
            ));
        }
        
        // Generate cache key
        let cache_key = format!("{}:{}", source, book_id);
        
        // Check cache first
        if let Some(cached) = self.get_cached_detail(&cache_key) {
            tracing::debug!("Cache hit for book detail: {}:{}", source, book_id);
            return Ok(cached);
        }

        // Resolve source ID (in case it's just a name)
        let source_id = self.select_scraper(Some(source)).await?;
        
        tracing::debug!("Cache miss for book detail: {}:{}, calling plugin {}", source, book_id, source_id);
        
        // Call scraper plugin
        let params = json!({
            "book_id": book_id,
        });
        
        let result = self.plugin_manager
            .call_scraper(&source_id, ScraperMethod::GetDetail, params)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get book detail from {}: {}", source_id, e);
                TingError::PluginExecutionError(
                    format!("Failed to get book detail: {}", e)
                )
            })?;
        
        // Parse result
        let book_detail: crate::plugin::scraper::BookDetail = 
            serde_json::from_value(result)
                .map_err(|e| TingError::DeserializationError(
                    format!("Failed to parse book detail: {}", e)
                ))?;
        
        // Cache the result
        self.cache_detail(&cache_key, book_detail.clone());
        
        Ok(book_detail)
    }
    
    /// Try to find a fallback scraper (different from the failed one)
    async fn try_fallback_scraper(&self, failed_source: &str) -> Result<String> {
        let sources = self.get_sources().await;
        let fallback = sources.into_iter()
            .filter(|s| s.enabled && s.id != failed_source)
            .next();
        
        match fallback {
            Some(source) => {
                tracing::info!("Using fallback scraper: {}", source.id);
                Ok(source.id)
            }
            None => Err(TingError::PluginNotFound(
                "No fallback scraper available".to_string()
            )),
        }
    }
    
    /// Get cached search result if not expired
    fn get_cached_search(&self, key: &str) -> Option<crate::plugin::scraper::SearchResult> {
        let cache = self.search_cache.read().ok()?;
        let entry = cache.get(key)?;
        
        if Instant::now() < entry.expires_at {
            Some(entry.data.clone())
        } else {
            None
        }
    }
    
    /// Cache search result
    fn cache_search_result(&self, key: &str, result: crate::plugin::scraper::SearchResult) {
        if let Ok(mut cache) = self.search_cache.write() {
            cache.insert(key.to_string(), CacheEntry {
                data: result,
                expires_at: Instant::now() + self.cache_ttl,
            });
            
            // Clean up expired entries if cache is getting large
            if cache.len() > 100 {
                cache.retain(|_, entry| Instant::now() < entry.expires_at);
            }
        }
    }
    
    /// Get cached book detail if not expired
    fn get_cached_detail(&self, key: &str) -> Option<crate::plugin::scraper::BookDetail> {
        let cache = self.detail_cache.read().ok()?;
        let entry = cache.get(key)?;
        
        if Instant::now() < entry.expires_at {
            Some(entry.data.clone())
        } else {
            None
        }
    }
    
    /// Cache book detail
    fn cache_detail(&self, key: &str, detail: crate::plugin::scraper::BookDetail) {
        if let Ok(mut cache) = self.detail_cache.write() {
            cache.insert(key.to_string(), CacheEntry {
                data: detail,
                expires_at: Instant::now() + self.cache_ttl,
            });
            
            // Clean up expired entries if cache is getting large
            if cache.len() > 100 {
                cache.retain(|_, entry| Instant::now() < entry.expires_at);
            }
        }
    }
    
    /// Clear all caches
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.search_cache.write() {
            cache.clear();
        }
        if let Ok(mut cache) = self.detail_cache.write() {
            cache.clear();
        }
        tracing::info!("Scraper service cache cleared");
    }
    
    /// Get cache statistics
    pub fn get_cache_stats(&self) -> (usize, usize) {
        let search_count = self.search_cache.read()
            .map(|c| c.len())
            .unwrap_or(0);
        let detail_count = self.detail_cache.read()
            .map(|c| c.len())
            .unwrap_or(0);
        (search_count, detail_count)
    }

    /// Scrape book metadata using the provided configuration strategy
    pub async fn scrape_book_metadata(
        &self,
        query: &str,
        config: &crate::db::models::ScraperConfig,
    ) -> Result<crate::plugin::scraper::BookDetail> {
        use std::collections::HashSet;
        
        // 1. Identify all unique sources we need to query
        let mut all_sources = HashSet::new();
        
        // Add default sources
        for s in &config.default_sources {
            all_sources.insert(s.clone());
        }
        
        // Add specific field sources
        if let Some(sources) = &config.author_sources { sources.iter().for_each(|s| { all_sources.insert(s.clone()); }); }
        if let Some(sources) = &config.narrator_sources { sources.iter().for_each(|s| { all_sources.insert(s.clone()); }); }
        if let Some(sources) = &config.cover_sources { sources.iter().for_each(|s| { all_sources.insert(s.clone()); }); }
        if let Some(sources) = &config.intro_sources { sources.iter().for_each(|s| { all_sources.insert(s.clone()); }); }
        if let Some(sources) = &config.tags_sources { sources.iter().for_each(|s| { all_sources.insert(s.clone()); }); }
        
        // If no sources configured, try to find any active scraper
        if all_sources.is_empty() {
             match self.select_scraper(None).await {
                 Ok(s) => { all_sources.insert(s); },
                 Err(_) => return Err(TingError::NotFound("No active scraper plugins available".to_string())),
             }
        }
        
        // 2. Fetch results from all required sources
        let mut source_results: HashMap<String, crate::plugin::scraper::BookDetail> = HashMap::new();
        
        for source_id in all_sources {
            // Search (page 1, limit 1)
            match self.search(query, Some(&source_id), 1, 1).await {
                Ok(search_res) => {
                    if let Some(item) = search_res.items.first() {
                        // We only use the search result. 
                        // Plugins are responsible for returning complete metadata in search results
                        // or handling multi-step scraping internally if needed.
                        let detail = crate::plugin::scraper::BookDetail {
                            id: item.id.clone(),
                            title: item.title.clone(),
                            author: item.author.clone(),
                            narrator: item.narrator.clone(),
                            cover_url: item.cover_url.clone(),
                            intro: item.intro.clone().unwrap_or_default(),
                            tags: item.tags.clone(),
                            chapter_count: item.chapter_count.unwrap_or(0),
                            duration: item.duration.clone(),
                        };
                        
                        source_results.insert(source_id, detail);
                    } else {
                        tracing::debug!("No search results from {} for {}", source_id, query);
                    }
                },
                Err(e) => tracing::warn!("Search failed on {}: {}", source_id, e),
            }
        }
        
        if source_results.is_empty() {
            return Err(TingError::NotFound("No metadata found from any scraper".to_string()));
        }
        
        // 3. Merge results based on priority
        let mut final_detail = crate::plugin::scraper::BookDetail {
            id: "".to_string(),
            title: query.to_string(),
            author: "".to_string(),
            narrator: None,
            cover_url: None,
            intro: "".to_string(),
            tags: vec![],
            chapter_count: 0,
            duration: None,
        };
        
        // Helper to get effective sources (specific + default fallback)
        macro_rules! get_effective_sources {
            ($specific:expr) => {{
                let mut sources = Vec::new();
                // First try specific sources if configured
                if let Some(s) = $specific {
                    sources.extend(s);
                }
                // Then fallback to default sources
                for ds in &config.default_sources {
                    if !sources.contains(&ds) {
                        sources.push(ds);
                    }
                }
                sources
            }};
        }

        // Title (Use default/first available)
        // Since we removed title_sources, we just pick the title from the highest priority source that has one
        for source in &config.default_sources {
            if let Some(detail) = source_results.get(source) {
                if !detail.title.is_empty() {
                    final_detail.title = detail.title.clone();
                    break;
                }
            }
        }
        // If still empty, try any available source result
        if final_detail.title == query { // Assuming query is the fallback
             for detail in source_results.values() {
                 if !detail.title.is_empty() {
                     final_detail.title = detail.title.clone();
                     break;
                 }
             }
        }

        // Author
        for source in get_effective_sources!(config.author_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if !detail.author.is_empty() {
                    final_detail.author = detail.author.clone();
                    break;
                }
            }
        }

        // Narrator
        for source in get_effective_sources!(config.narrator_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if detail.narrator.is_some() {
                    final_detail.narrator = detail.narrator.clone();
                    break;
                }
            }
        }

        // Cover
        for source in get_effective_sources!(config.cover_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if detail.cover_url.is_some() {
                    final_detail.cover_url = detail.cover_url.clone();
                    break;
                }
            }
        }

        // Intro
        for source in get_effective_sources!(config.intro_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if !detail.intro.is_empty() {
                    final_detail.intro = detail.intro.clone();
                    break;
                }
            }
        }

        // Tags
        for source in get_effective_sources!(config.tags_sources.as_ref()) {
            if let Some(detail) = source_results.get(source) {
                if !detail.tags.is_empty() {
                    final_detail.tags = detail.tags.clone();
                    break;
                }
            }
        }
        
        Ok(final_detail)
    }
}

pub struct FormatService;
