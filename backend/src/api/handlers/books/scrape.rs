use crate::api::models::{ScrapeBookRequest, ScrapeDiffRequest, ScrapeDiffResponse, ScrapeApplyRequest, BookResponse};
use crate::core::error::{Result, TingError};
use crate::db::models::ScraperConfig;
use crate::core::nfo_manager::BookMetadata;
use crate::db::repository::{Repository, ChapterRepository};
use axum::{extract::{Path, State}, response::IntoResponse, Json};
use super::AppState;

/// Handler for POST /api/v1/books/:id/scrape - Scrape and update book details
pub async fn scrape_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ScrapeBookRequest>,
) -> Result<impl IntoResponse> {
    let existing_book = state
        .book_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    let (source, external_id) = match (req.source, req.external_id) {
        (Some(s), Some(e)) => (s, e),
        _ => return Err(TingError::ValidationError("Source and External ID are required for now".to_string())),
    };

    let detail = state.scraper_service.get_detail(&source, &external_id).await?;

    let mut updated_book = existing_book.clone();
    
    updated_book.title = Some(detail.title);
    updated_book.author = Some(detail.author);

    if let Some(narrator) = detail.narrator {
        updated_book.narrator = Some(narrator);
    }
    
    if !detail.intro.is_empty() {
        updated_book.description = Some(detail.intro);
    }
    
    if !detail.tags.is_empty() {
        updated_book.tags = Some(detail.tags.join(","));
    }

    let old_cover = existing_book.cover_url.clone();
    let new_cover = detail.cover_url;
    
    if let Some(url) = new_cover {
        updated_book.cover_url = Some(url.clone());
    }

    let should_calculate_color = updated_book.cover_url != old_cover || updated_book.theme_color.is_none();
    
    if should_calculate_color {
        if let Some(ref url) = updated_book.cover_url {
             let cover_path = if url.starts_with("http://") || url.starts_with("https://") {
                 url.clone()
             } else {
                 let book_path = std::path::Path::new(&updated_book.path);
                 if std::path::Path::new(&url).is_absolute() {
                     url.clone()
                 } else {
                     book_path.join(&url).to_string_lossy().to_string()
                 }
             };

             match crate::core::color::calculate_theme_color(&cover_path).await {
                 Ok(Some(color)) => {
                     updated_book.theme_color = Some(color);
                 },
                 Ok(None) => {
                     if let Ok(Some(library)) = state.library_repo.find_by_id(&updated_book.library_id).await {
                         if library.library_type == "webdav" {
                             if let Ok((mut reader, _)) = state.storage_service.get_webdav_reader(
                                 &library, 
                                 &url, 
                                 None, 
                                 state.encryption_key.as_ref()
                             ).await {
                                 let mut buffer = Vec::new();
                                 if tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buffer).await.is_ok() {
                                     if let Ok(Some(color)) = crate::core::color::calculate_theme_color_from_bytes(&buffer).await {
                                         updated_book.theme_color = Some(color);
                                     }
                                 }
                             }
                         }
                     }
                 },
                 Err(_) => {}
             }
        }
    }

    state.book_repo.update(&updated_book).await?;

    // Check NFO writing
    if let Ok(Some(library)) = state.library_repo.find_by_id(&updated_book.library_id).await {
        let config: crate::db::models::ScraperConfig = library.scraper_config
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
            
        // Determine path (shared for NFO and metadata.json)
        let target_dir = if library.library_type == "webdav" {
            // WebDAV uses hash-based temp dir
            let mut hasher = sha2::Sha256::new();
            use sha2::Digest;
            hasher.update(updated_book.path.as_bytes());
            let book_hash = format!("{:x}", hasher.finalize());
            let temp_book_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("temp").join(&book_hash);
            if !temp_book_dir.exists() { std::fs::create_dir_all(&temp_book_dir).ok(); }
            temp_book_dir
        } else {
            std::path::PathBuf::from(&updated_book.path)
        };

        // Handle NFO writing (Local & WebDAV)
        if config.nfo_writing_enabled {
            let mut metadata = BookMetadata::new(
                updated_book.title.clone().unwrap_or_default(),
                "ting-reader".to_string(),
                updated_book.id.clone(),
                0,
            );
            metadata.author = updated_book.author.clone();
            metadata.narrator = updated_book.narrator.clone();
            metadata.intro = updated_book.description.clone();
            metadata.cover_url = updated_book.cover_url.clone();
            if let Some(tags_str) = &updated_book.tags {
                 metadata.tags.items = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            }
            metadata.touch(); // Update timestamp
            
            if let Err(e) = state.nfo_manager.write_book_nfo_to_dir(&target_dir, &metadata) {
                tracing::warn!("为书籍 {} 写入 NFO 失败: {}", updated_book.title.as_deref().unwrap_or("?"), e);
            }
        }

        // Handle metadata.json writing
        if config.metadata_writing_enabled {
            // Read existing metadata.json to preserve extended fields
            let mut metadata_json = crate::core::metadata_writer::read_metadata_json(&target_dir).unwrap_or(None).unwrap_or_default();
            
            // Update fields from book record
            metadata_json.title = updated_book.title.clone();
            metadata_json.authors = updated_book.author.clone().map(|s| vec![s]).unwrap_or_default();
            metadata_json.narrators = updated_book.narrator.clone().map(|s| vec![s]).unwrap_or_default();
            metadata_json.description = updated_book.description.clone();
            metadata_json.genres = updated_book.genre.clone().map(|s| s.split(',').map(|t| t.trim().to_string()).collect()).unwrap_or_default();
            metadata_json.tags = updated_book.tags.clone().map(|s| s.split(',').map(|t| t.trim().to_string()).collect()).unwrap_or_default();
            metadata_json.published_year = updated_book.year.map(|y| y.to_string());
            
            // Subtitle is now in metadata.json but not in Book struct, so we preserve what was read.
            // If request had extended fields (not supported in UpdateBookRequest yet), we would update them here.
            
            if let Err(e) = crate::core::metadata_writer::write_metadata_json(&target_dir, &metadata_json) {
                tracing::error!(target: "audit::metadata", "为书籍 {} 写入 metadata.json 失败: {}", updated_book.title.as_deref().unwrap_or("?"), e);
            }
        }
    }

    Ok(Json(BookResponse::from(updated_book)))
}


/// Handler for POST /api/v1/books/:id/scrape-diff - Get scrape diff
pub async fn scrape_book_diff(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<ScrapeDiffRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let existing_book = state.book_repo.find_by_id(&id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    // 1. Get Library Config
    let library = state.library_repo.find_by_id(&existing_book.library_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Library with id {} not found", existing_book.library_id)))?;
    
    let config: ScraperConfig = if let Some(json) = &library.scraper_config {
        serde_json::from_str(json).unwrap_or_default()
    } else {
        ScraperConfig::default()
    };

    // 2. Determine Primary Source (from Default List or First Enabled)
    let sources = state.scraper_service.get_sources().await;
    
    // Find first enabled source from config defaults, or just first enabled system-wide
    let mut primary_source_id = None;
    for source_id in &config.default_sources {
        if let Some(s) = sources.iter().find(|s| s.id == *source_id && s.enabled) {
            primary_source_id = Some(s.id.clone());
            break;
        }
    }
    
    if primary_source_id.is_none() {
        primary_source_id = sources.iter().find(|s| s.enabled).map(|s| s.id.clone());
    }
    
    let primary_source_id = match primary_source_id {
        Some(s) => s,
        None => return Err(TingError::PluginNotFound("No active scraper plugins found".to_string())),
    };

    // 3. Search and Get Detail from Primary Source
    // Use scraper service search to find match by title
    let search_results = state.scraper_service.search(
        &req.query, 
        req.author.as_deref(),
        req.narrator.as_deref(),
        Some(&primary_source_id), 
        1, 
        1
    ).await?;
    
    if search_results.items.is_empty() {
        return Err(TingError::NotFound("No matching book found in scrapers".to_string()));
    }
    
    let best_match = &search_results.items[0];
    // We use search result directly as it now contains full metadata
    let mut detail = crate::plugin::scraper::BookDetail {
        id: best_match.id.clone(),
        title: best_match.title.clone(),
        author: best_match.author.clone(),
        narrator: best_match.narrator.clone(),
        cover_url: best_match.cover_url.clone(),
        intro: best_match.intro.clone().unwrap_or_default(),
        tags: best_match.tags.clone(),
        chapter_count: best_match.chapter_count.unwrap_or(0),
        duration: best_match.duration,
        subtitle: best_match.subtitle.clone(),
        published_year: best_match.published_year.clone(),
        published_date: best_match.published_date.clone(),
        publisher: best_match.publisher.clone(),
        isbn: best_match.isbn.clone(),
        asin: best_match.asin.clone(),
        language: best_match.language.clone(),
        explicit: best_match.explicit.unwrap_or(false),
        abridged: best_match.abridged.unwrap_or(false),
        genre: None,
    };
    
    // 4. Handle Specific Field Sources (Merge Strategy)
    // Helper to fetch and merge specific field
    // We assume the search query is the same (Book Title) for other sources too.
    
    // Check Author Source
    if let Some(srcs) = &config.author_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
             if let Some(s_id) = sources.iter().find(|s| s.id == srcs[0] && s.enabled).map(|s| s.id.clone()) {
                 if let Ok(res) = state.scraper_service.search(
                     &req.query, 
                     req.author.as_deref(),
                     req.narrator.as_deref(),
                     Some(&s_id), 
                     1, 
                     1
                 ).await {
                     if !res.items.is_empty() {
                         let item = &res.items[0];
                         if !item.author.is_empty() { detail.author = item.author.clone(); }
                     }
                 }
             }
        }
    }

    // Check Narrator Source
    if let Some(srcs) = &config.narrator_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
             if let Some(s_id) = sources.iter().find(|s| s.id == srcs[0] && s.enabled).map(|s| s.id.clone()) {
                 if let Ok(res) = state.scraper_service.search(
                     &req.query, 
                     req.author.as_deref(),
                     req.narrator.as_deref(),
                     Some(&s_id), 
                     1, 
                     1
                 ).await {
                     if !res.items.is_empty() {
                         let item = &res.items[0];
                         if item.narrator.is_some() { detail.narrator = item.narrator.clone(); }
                     }
                 }
             }
        }
    }

    // Check Cover Source
    if let Some(srcs) = &config.cover_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
             if let Some(s_id) = sources.iter().find(|s| s.id == srcs[0] && s.enabled).map(|s| s.id.clone()) {
                 if let Ok(res) = state.scraper_service.search(
                     &req.query, 
                     req.author.as_deref(),
                     req.narrator.as_deref(),
                     Some(&s_id), 
                     1, 
                     1
                 ).await {
                     if !res.items.is_empty() {
                         let item = &res.items[0];
                         if item.cover_url.is_some() { detail.cover_url = item.cover_url.clone(); }
                     }
                 }
             }
        }
    }

    // Check Intro Source
    if let Some(srcs) = &config.intro_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
             if let Some(s_id) = sources.iter().find(|s| s.id == srcs[0] && s.enabled).map(|s| s.id.clone()) {
                 if let Ok(res) = state.scraper_service.search(
                     &req.query, 
                     req.author.as_deref(),
                     req.narrator.as_deref(),
                     Some(&s_id), 
                     1, 
                     1
                 ).await {
                     if !res.items.is_empty() {
                         let item = &res.items[0];
                         if let Some(intro) = &item.intro {
                             if !intro.is_empty() { detail.intro = intro.clone(); }
                         }
                     }
                 }
             }
        }
    }

    // Check Tags Source
    if let Some(srcs) = &config.tags_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
             if let Some(s_id) = sources.iter().find(|s| s.id == srcs[0] && s.enabled).map(|s| s.id.clone()) {
                 if let Ok(res) = state.scraper_service.search(
                     &req.query, 
                     req.author.as_deref(),
                     req.narrator.as_deref(),
                     Some(&s_id), 
                     1, 
                     1
                 ).await {
                     if !res.items.is_empty() {
                         let item = &res.items[0];
                         if !item.tags.is_empty() { detail.tags = item.tags.clone(); }
                     }
                 }
             }
        }
    }
    
    // Construct ScrapeMetadata for current book
    let current_meta = crate::api::models::books::ScrapeMetadata {
        title: existing_book.title.clone().unwrap_or_default(),
        author: existing_book.author.clone().unwrap_or_default(),
        narrator: existing_book.narrator.clone().unwrap_or_default(),
        description: existing_book.description.clone().unwrap_or_default(),
        cover_url: existing_book.cover_url.clone(),
        tags: existing_book.tags.map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()),
        genre: existing_book.genre.clone(),
    };
    
    // Construct ScrapeMetadata for scraped detail
    let clean_cover_url = detail.cover_url.clone();

    let scraped_meta = crate::api::models::books::ScrapeMetadata {
        title: detail.title.clone(),
        author: detail.author.clone(),
        narrator: detail.narrator.clone().unwrap_or_default(),
        description: detail.intro.clone(),
        cover_url: clean_cover_url,
        tags: if detail.tags.is_empty() { None } else { Some(detail.tags.clone()) },
        genre: detail.genre.clone(),
    };
    
    // Chapter changes (Not implemented yet, returning empty list)
    // To implement this, we need ScraperService to return chapter list
    let chapter_changes = Vec::new();

    Ok(Json(ScrapeDiffResponse {
        current: current_meta,
        scraped: scraped_meta,
        chapter_changes,
    }))
}


/// Handler for POST /api/v1/books/:id/scrape-apply - Apply scrape result
pub async fn apply_scrape_result(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<ScrapeApplyRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let mut book = state.book_repo.find_by_id(&id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    if req.apply_metadata {
        let detail = &req.metadata;
        
        if !detail.title.is_empty() { book.title = Some(detail.title.clone()); }
        if !detail.author.is_empty() { book.author = Some(detail.author.clone()); }
        if let Some(n) = &detail.narrator { book.narrator = Some(n.clone()); }
        if !detail.intro.is_empty() { book.description = Some(detail.intro.clone()); }
        if !detail.tags.is_empty() { book.tags = Some(detail.tags.join(",")); }
        if let Some(g) = &detail.genre { book.genre = Some(g.clone()); }
        if let Some(url) = &detail.cover_url { 
            book.cover_url = Some(url.clone());
            
            // Handle referer for internal processing if present
            let mut internal_url = url.clone();
            if let Some(idx) = internal_url.find("#referer=") {
                internal_url = internal_url[..idx].to_string();
            }

            // Recalculate theme color for new cover
            match crate::core::color::calculate_theme_color(&url).await {
                Ok(Some(color)) => {
                    tracing::info!("更新了书籍 {} 的主题颜色: {}", book.id, color);
                    book.theme_color = Some(color);
                },
                Ok(None) => {
                     // Try WebDAV if local/http failed and it's a webdav library
                     if let Ok(Some(library)) = state.library_repo.find_by_id(&book.library_id).await {
                         if library.library_type == "webdav" {
                             if let Ok((mut reader, _)) = state.storage_service.get_webdav_reader(
                                 &library, 
                                 &internal_url, 
                                 None, 
                                 state.encryption_key.as_ref()
                             ).await {
                                 let mut buffer = Vec::new();
                                 if tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buffer).await.is_ok() {
                                     if let Ok(Some(color)) = crate::core::color::calculate_theme_color_from_bytes(&buffer).await {
                                         book.theme_color = Some(color);
                                     }
                                 }
                             }
                         }
                     }
                },
                Err(e) => {
                    tracing::warn!("计算主题颜色失败: {}", e);
                }
            }
        }
        
        // Set manual corrected
        book.manual_corrected = 1; // 1 for true
        book.match_pattern = Some(regex::escape(&detail.title)); // Set default match pattern to exact title
        
        state.book_repo.update(&book).await?;

        // Check NFO writing
    if let Ok(Some(library)) = state.library_repo.find_by_id(&book.library_id).await {
        let config: crate::db::models::ScraperConfig = library.scraper_config
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
            
        // Determine path (shared for NFO and metadata.json)
        let target_dir = if library.library_type == "webdav" {
            // WebDAV uses hash-based temp dir
            let mut hasher = sha2::Sha256::new();
            use sha2::Digest;
            hasher.update(book.path.as_bytes());
            let book_hash = format!("{:x}", hasher.finalize());
            let temp_book_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("temp").join(&book_hash);
            if !temp_book_dir.exists() { std::fs::create_dir_all(&temp_book_dir).ok(); }
            temp_book_dir
        } else {
            std::path::PathBuf::from(&book.path)
        };

        // Handle NFO writing (Local & WebDAV)
        if config.nfo_writing_enabled {
            let mut metadata = BookMetadata::new(
                book.title.clone().unwrap_or_default(),
                "ting-reader".to_string(),
                book.id.clone(),
                0,
            );
            metadata.author = book.author.clone();
            metadata.narrator = book.narrator.clone();
            metadata.intro = book.description.clone();
            metadata.cover_url = book.cover_url.clone();
            if let Some(tags_str) = &book.tags {
                 metadata.tags.items = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            }
            metadata.touch();
            
            if let Err(e) = state.nfo_manager.write_book_nfo_to_dir(&target_dir, &metadata) {
                tracing::warn!("为书籍 {} 写入 NFO 失败: {}", book.title.as_deref().unwrap_or("?"), e);
            }
        }

        // Handle metadata.json writing
        if config.metadata_writing_enabled {
            // Read existing metadata.json to preserve extended fields
            let mut metadata_json = crate::core::metadata_writer::read_metadata_json(&target_dir).unwrap_or(None).unwrap_or_default();
            
            // Update fields from book record
            metadata_json.title = book.title.clone();
            metadata_json.authors = book.author.clone().map(|s| vec![s]).unwrap_or_default();
            metadata_json.narrators = book.narrator.clone().map(|s| vec![s]).unwrap_or_default();
            metadata_json.description = book.description.clone();
            metadata_json.genres = book.genre.clone().map(|s| s.split(',').map(|t| t.trim().to_string()).collect()).unwrap_or_default();
            metadata_json.tags = book.tags.clone().map(|s| s.split(',').map(|t| t.trim().to_string()).collect()).unwrap_or_default();
            metadata_json.published_year = book.year.map(|y| y.to_string());
            
            // Sync chapters from DB
            let chapter_repo = ChapterRepository::new(state.book_repo.db().clone());
            if let Ok(chapters) = chapter_repo.find_by_book(&book.id).await {
                let mut sorted_chapters = chapters;
                sorted_chapters.sort_by(|a, b| {
                    a.chapter_index.unwrap_or(0).cmp(&b.chapter_index.unwrap_or(0))
                        .then_with(|| natord::compare(a.title.as_deref().unwrap_or(""), b.title.as_deref().unwrap_or("")))
                });

                let mut abs_chapters = Vec::new();
                let mut current_time = 0.0;
                for (idx, ch) in sorted_chapters.iter().enumerate() {
                    let duration = ch.duration.unwrap_or(0) as f64;
                    abs_chapters.push(crate::core::metadata_writer::AudiobookshelfChapter {
                        id: idx as u32,
                        start: current_time,
                        end: current_time + duration,
                        title: ch.title.clone().unwrap_or_default(),
                    });
                    current_time += duration;
                }
                metadata_json.chapters = abs_chapters;
            }
            
            // Sync series from DB
            let series_list = state.series_repo.find_series_by_book(&book.id).await.unwrap_or_default();
            let mut series_titles = Vec::new();
            for series in series_list {
                if let Ok(books) = state.series_repo.find_books_by_series(&series.id).await {
                    if let Some((_, order)) = books.iter().find(|(b, _)| b.id == book.id) {
                        series_titles.push(format!("{} #{}", series.title, order));
                    } else {
                        series_titles.push(series.title);
                    }
                } else {
                    series_titles.push(series.title);
                }
            }
            metadata_json.series = series_titles;
            
            // Apply scraped extended fields if available
            if !detail.subtitle.is_none() { metadata_json.subtitle = detail.subtitle.clone(); }
            if !detail.published_year.is_none() { metadata_json.published_year = detail.published_year.clone(); }
            if !detail.published_date.is_none() { metadata_json.published_date = detail.published_date.clone(); }
            if !detail.publisher.is_none() { metadata_json.publisher = detail.publisher.clone(); }
            if !detail.isbn.is_none() { metadata_json.isbn = detail.isbn.clone(); }
            if !detail.asin.is_none() { metadata_json.asin = detail.asin.clone(); }
            if !detail.language.is_none() { metadata_json.language = detail.language.clone(); }
            if detail.explicit { metadata_json.explicit = true; }
            if detail.abridged { metadata_json.abridged = true; }
            
            if let Err(e) = crate::core::metadata_writer::write_metadata_json(&target_dir, &metadata_json) {
                tracing::error!(target: "audit::metadata", "为书籍 {} 写入 metadata.json 失败: {}", book.title.as_deref().unwrap_or("?"), e);
            }
        }
    }
    }
    
    // Handle chapter updates if any (req.apply_chapters)
    // Since we don't have scraped chapters yet, we skip this for now.
    
    Ok(Json(BookResponse::from(book)))
}

