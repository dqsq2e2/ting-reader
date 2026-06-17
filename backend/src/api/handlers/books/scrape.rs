use super::AppState;
use crate::api::models::{BookResponse, ScrapeApplyRequest, ScrapeDiffRequest, ScrapeDiffResponse};
use crate::core::error::{Result, TingError};
use crate::core::nfo_manager::BookMetadata;
use crate::db::models::ScraperConfig;
use crate::db::repository::{ChapterRepository, Repository};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};

#[derive(Default)]
struct SelectedScrapeExtendedMetadata {
    subtitle: Option<String>,
    published_year: Option<String>,
    published_date: Option<String>,
    publisher: Option<String>,
    isbn: Option<String>,
    asin: Option<String>,
    language: Option<String>,
    explicit: Option<bool>,
    abridged: Option<bool>,
    duration: Option<u64>,
}

/// Handler for POST /api/v1/books/:id/scrape-diff - Get scrape diff
pub async fn scrape_book_diff(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<ScrapeDiffRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }

    let existing_book = state
        .book_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    // 1. Get Library Config
    let library = state
        .library_repo
        .find_by_id(&existing_book.library_id)
        .await?
        .ok_or_else(|| {
            TingError::NotFound(format!(
                "Library with id {} not found",
                existing_book.library_id
            ))
        })?;

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
        None => {
            return Err(TingError::PluginNotFound(
                "No active scraper plugins found".to_string(),
            ))
        }
    };

    // 3. Search and Get Detail from Primary Source
    // Use scraper service search to find match by title
    let search_results = state
        .scraper_service
        .search(
            &req.query,
            req.author.as_deref(),
            req.narrator.as_deref(),
            Some(&primary_source_id),
            1,
            1,
        )
        .await?;

    if search_results.items.is_empty() {
        return Err(TingError::NotFound(
            "No matching book found in scrapers".to_string(),
        ));
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
        chapter_count: 0,
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
        genre: best_match.genre.clone(),
    };

    // 4. Handle Specific Field Sources (Merge Strategy)
    // Helper to fetch and merge specific field
    // We assume the search query is the same (Book Title) for other sources too.

    // Check Author Source
    if let Some(srcs) = &config.author_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
            if let Some(s_id) = sources
                .iter()
                .find(|s| s.id == srcs[0] && s.enabled)
                .map(|s| s.id.clone())
            {
                if let Ok(res) = state
                    .scraper_service
                    .search(
                        &req.query,
                        req.author.as_deref(),
                        req.narrator.as_deref(),
                        Some(&s_id),
                        1,
                        1,
                    )
                    .await
                {
                    if !res.items.is_empty() {
                        let item = &res.items[0];
                        if !item.author.is_empty() {
                            detail.author = item.author.clone();
                        }
                    }
                }
            }
        }
    }

    // Check Narrator Source
    if let Some(srcs) = &config.narrator_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
            if let Some(s_id) = sources
                .iter()
                .find(|s| s.id == srcs[0] && s.enabled)
                .map(|s| s.id.clone())
            {
                if let Ok(res) = state
                    .scraper_service
                    .search(
                        &req.query,
                        req.author.as_deref(),
                        req.narrator.as_deref(),
                        Some(&s_id),
                        1,
                        1,
                    )
                    .await
                {
                    if !res.items.is_empty() {
                        let item = &res.items[0];
                        if item.narrator.is_some() {
                            detail.narrator = item.narrator.clone();
                        }
                    }
                }
            }
        }
    }

    // Check Cover Source
    if let Some(srcs) = &config.cover_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
            if let Some(s_id) = sources
                .iter()
                .find(|s| s.id == srcs[0] && s.enabled)
                .map(|s| s.id.clone())
            {
                if let Ok(res) = state
                    .scraper_service
                    .search(
                        &req.query,
                        req.author.as_deref(),
                        req.narrator.as_deref(),
                        Some(&s_id),
                        1,
                        1,
                    )
                    .await
                {
                    if !res.items.is_empty() {
                        let item = &res.items[0];
                        if item.cover_url.is_some() {
                            detail.cover_url = item.cover_url.clone();
                        }
                    }
                }
            }
        }
    }

    // Check Intro Source
    if let Some(srcs) = &config.intro_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
            if let Some(s_id) = sources
                .iter()
                .find(|s| s.id == srcs[0] && s.enabled)
                .map(|s| s.id.clone())
            {
                if let Ok(res) = state
                    .scraper_service
                    .search(
                        &req.query,
                        req.author.as_deref(),
                        req.narrator.as_deref(),
                        Some(&s_id),
                        1,
                        1,
                    )
                    .await
                {
                    if !res.items.is_empty() {
                        let item = &res.items[0];
                        if let Some(intro) = &item.intro {
                            if !intro.is_empty() {
                                detail.intro = intro.clone();
                            }
                        }
                    }
                }
            }
        }
    }

    // Check Tags Source
    if let Some(srcs) = &config.tags_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
            if let Some(s_id) = sources
                .iter()
                .find(|s| s.id == srcs[0] && s.enabled)
                .map(|s| s.id.clone())
            {
                if let Ok(res) = state
                    .scraper_service
                    .search(
                        &req.query,
                        req.author.as_deref(),
                        req.narrator.as_deref(),
                        Some(&s_id),
                        1,
                        1,
                    )
                    .await
                {
                    if !res.items.is_empty() {
                        let item = &res.items[0];
                        if !item.tags.is_empty() {
                            detail.tags = item.tags.clone();
                        }
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
        tags: existing_book.tags.map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        }),
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
        tags: if detail.tags.is_empty() {
            None
        } else {
            Some(detail.tags.clone())
        },
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
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }

    let mut book = state
        .book_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    if let Some(fields) = &req.fields {
        let mut extended = SelectedScrapeExtendedMetadata::default();
        let mut has_extended = false;

        for (field, selection) in fields {
            match field.as_str() {
                "title" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        book.title = Some(value);
                    }
                }
                "author" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        book.author = Some(value);
                    }
                }
                "narrator" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        book.narrator = Some(value);
                    }
                }
                "description" | "intro" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        book.description = Some(value);
                    }
                }
                "cover_url" | "coverUrl" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        book.cover_url = Some(value.clone());
                        recalculate_cover_theme_color(&state, &mut book, &value).await;
                    }
                }
                "tags" => {
                    if let Some(value) = scrape_value_to_tags(&selection.value) {
                        book.tags = Some(value);
                    }
                }
                "genre" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        book.genre = Some(value);
                    }
                }
                "year" | "published_year" | "publishedYear" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        if let Ok(year) = value.parse::<i32>() {
                            book.year = Some(year);
                        }
                        extended.published_year = Some(value);
                        has_extended = true;
                    } else if let Some(year) = selection.value.as_i64() {
                        book.year = Some(year as i32);
                        extended.published_year = Some(year.to_string());
                        has_extended = true;
                    }
                }
                "subtitle" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        extended.subtitle = Some(value);
                        has_extended = true;
                    }
                }
                "published_date" | "publishedDate" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        extended.published_date = Some(value);
                        has_extended = true;
                    }
                }
                "publisher" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        extended.publisher = Some(value);
                        has_extended = true;
                    }
                }
                "isbn" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        extended.isbn = Some(value);
                        has_extended = true;
                    }
                }
                "asin" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        extended.asin = Some(value);
                        has_extended = true;
                    }
                }
                "language" => {
                    if let Some(value) = scrape_value_to_string(&selection.value) {
                        extended.language = Some(value);
                        has_extended = true;
                    }
                }
                "explicit" => {
                    if let Some(value) = scrape_value_to_bool(&selection.value) {
                        extended.explicit = Some(value);
                        has_extended = true;
                    }
                }
                "abridged" => {
                    if let Some(value) = scrape_value_to_bool(&selection.value) {
                        extended.abridged = Some(value);
                        has_extended = true;
                    }
                }
                "duration" => {
                    if let Some(value) = scrape_value_to_u64(&selection.value) {
                        extended.duration = Some(value);
                        has_extended = true;
                    }
                }
                _ => {}
            }
        }

        state.book_repo.update(&book).await?;
        sync_manual_scrape_lock(&state, &mut book).await?;
        sync_basic_scrape_outputs(&state, &book).await?;
        if has_extended {
            sync_scrape_extended_metadata(&state, &book, &extended).await?;
        }

        return Ok(Json(BookResponse::from(book)));
    }

    if req.apply_metadata {
        let detail = req.metadata.as_ref().ok_or_else(|| {
            TingError::ValidationError(
                "metadata is required when apply_metadata is true".to_string(),
            )
        })?;

        if !detail.title.is_empty() {
            book.title = Some(detail.title.clone());
        }
        if !detail.author.is_empty() {
            book.author = Some(detail.author.clone());
        }
        if let Some(n) = &detail.narrator {
            book.narrator = Some(n.clone());
        }
        if !detail.intro.is_empty() {
            book.description = Some(detail.intro.clone());
        }
        if !detail.tags.is_empty() {
            book.tags = Some(detail.tags.join(","));
        }
        if let Some(g) = &detail.genre {
            book.genre = Some(g.clone());
        }
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
                }
                Ok(None) => {
                    // Try WebDAV if local/http failed and it's a webdav library
                    if let Ok(Some(library)) = state.library_repo.find_by_id(&book.library_id).await
                    {
                        if library.library_type == "webdav" {
                            if let Ok((mut reader, _)) = state
                                .storage_service
                                .get_webdav_reader(
                                    &library,
                                    &internal_url,
                                    None,
                                    state.encryption_key.as_ref(),
                                )
                                .await
                            {
                                let mut buffer = Vec::new();
                                if tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buffer)
                                    .await
                                    .is_ok()
                                {
                                    if let Ok(Some(color)) =
                                        crate::core::color::calculate_theme_color_from_bytes(
                                            &buffer,
                                        )
                                        .await
                                    {
                                        book.theme_color = Some(color);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("计算主题颜色失败: {}", e);
                }
            }
        }

        state.book_repo.update(&book).await?;
        sync_manual_scrape_lock(&state, &mut book).await?;

        // Check NFO writing
        if let Ok(Some(library)) = state.library_repo.find_by_id(&book.library_id).await {
            let config: crate::db::models::ScraperConfig = library
                .scraper_config
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
                let temp_book_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("temp")
                    .join(&book_hash);
                if !temp_book_dir.exists() {
                    std::fs::create_dir_all(&temp_book_dir).ok();
                }
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
                    metadata.tags.items = tags_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                metadata.touch();

                if let Err(e) = state
                    .nfo_manager
                    .write_book_nfo_to_dir(&target_dir, &metadata)
                {
                    tracing::warn!(
                        "为书籍 {} 写入 NFO 失败: {}",
                        book.title.as_deref().unwrap_or("?"),
                        e
                    );
                }
            }

            // Handle metadata.json writing
            if config.metadata_writing_enabled {
                // Read existing metadata.json to preserve extended fields
                let mut metadata_json =
                    crate::core::metadata_writer::read_metadata_json(&target_dir)
                        .unwrap_or(None)
                        .unwrap_or_default();

                // Update fields from book record
                metadata_json.title = book.title.clone();
                metadata_json.authors = book.author.clone().map(|s| vec![s]).unwrap_or_default();
                metadata_json.narrators =
                    book.narrator.clone().map(|s| vec![s]).unwrap_or_default();
                metadata_json.description = book.description.clone();
                metadata_json.genres = book
                    .genre
                    .clone()
                    .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                    .unwrap_or_default();
                metadata_json.tags = book
                    .tags
                    .clone()
                    .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                    .unwrap_or_default();
                metadata_json.published_year = book.year.map(|y| y.to_string());

                // Sync chapters from DB
                let chapter_repo = ChapterRepository::new(state.book_repo.db().clone());
                if let Ok(chapters) = chapter_repo.find_by_book(&book.id).await {
                    let mut sorted_chapters = chapters;
                    sorted_chapters.sort_by(|a, b| {
                        a.chapter_index
                            .unwrap_or(0)
                            .cmp(&b.chapter_index.unwrap_or(0))
                            .then_with(|| {
                                natord::compare(
                                    a.title.as_deref().unwrap_or(""),
                                    b.title.as_deref().unwrap_or(""),
                                )
                            })
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
                let series_list = state
                    .series_repo
                    .find_series_by_book(&book.id)
                    .await
                    .unwrap_or_default();
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
                if !detail.subtitle.is_none() {
                    metadata_json.subtitle = detail.subtitle.clone();
                }
                if !detail.published_year.is_none() {
                    metadata_json.published_year = detail.published_year.clone();
                }
                if !detail.published_date.is_none() {
                    metadata_json.published_date = detail.published_date.clone();
                }
                if !detail.publisher.is_none() {
                    metadata_json.publisher = detail.publisher.clone();
                }
                if !detail.isbn.is_none() {
                    metadata_json.isbn = detail.isbn.clone();
                }
                if !detail.asin.is_none() {
                    metadata_json.asin = detail.asin.clone();
                }
                if !detail.language.is_none() {
                    metadata_json.language = detail.language.clone();
                }
                if detail.explicit {
                    metadata_json.explicit = true;
                }
                if detail.abridged {
                    metadata_json.abridged = true;
                }

                if let Err(e) =
                    crate::core::metadata_writer::write_metadata_json(&target_dir, &metadata_json)
                {
                    tracing::error!(target: "audit::metadata", "为书籍 {} 写入 metadata.json 失败: {}", book.title.as_deref().unwrap_or("?"), e);
                }
            }
        }
    }

    // Handle chapter updates if any (req.apply_chapters)
    // Since we don't have scraped chapters yet, we skip this for now.

    Ok(Json(BookResponse::from(book)))
}

fn scrape_value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn scrape_value_to_tags(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(items) => {
            let tags: Vec<String> = items
                .iter()
                .filter_map(scrape_value_to_string)
                .filter(|s| !s.trim().is_empty())
                .collect();
            if tags.is_empty() {
                None
            } else {
                Some(tags.join(","))
            }
        }
        _ => scrape_value_to_string(value),
    }
}

fn scrape_value_to_bool(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::String(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "y" | "是" => Some(true),
                "false" | "0" | "no" | "n" | "否" => Some(false),
                _ => None,
            }
        }
        serde_json::Value::Number(value) => value.as_i64().map(|value| value != 0),
        _ => None,
    }
}

fn scrape_value_to_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(value) => value.as_u64(),
        serde_json::Value::String(value) => value.trim().parse::<u64>().ok(),
        _ => None,
    }
}

async fn recalculate_cover_theme_color(
    state: &AppState,
    book: &mut crate::db::models::Book,
    url: &str,
) {
    let mut internal_url = url.to_string();
    if let Some(idx) = internal_url.find("#referer=") {
        internal_url = internal_url[..idx].to_string();
    }

    match crate::core::color::calculate_theme_color(url).await {
        Ok(Some(color)) => {
            book.theme_color = Some(color);
        }
        Ok(None) => {
            if let Ok(Some(library)) = state.library_repo.find_by_id(&book.library_id).await {
                if library.library_type == "webdav" {
                    if let Ok((mut reader, _)) = state
                        .storage_service
                        .get_webdav_reader(
                            &library,
                            &internal_url,
                            None,
                            state.encryption_key.as_ref(),
                        )
                        .await
                    {
                        let mut buffer = Vec::new();
                        if tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buffer)
                            .await
                            .is_ok()
                        {
                            if let Ok(Some(color)) =
                                crate::core::color::calculate_theme_color_from_bytes(&buffer).await
                            {
                                book.theme_color = Some(color);
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("璁＄畻涓婚棰滆壊澶辫触: {}", e);
        }
    }
}

async fn sync_manual_scrape_lock(
    state: &AppState,
    book: &mut crate::db::models::Book,
) -> Result<()> {
    let match_pattern = book
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(regex::escape);

    state
        .merge_service
        .update_manual_correction(&book.id, true, match_pattern)
        .await?;

    if let Some(updated_book) = state.book_repo.find_by_id(&book.id).await? {
        *book = updated_book;
    }

    Ok(())
}

async fn sync_basic_scrape_outputs(state: &AppState, book: &crate::db::models::Book) -> Result<()> {
    let Some(library) = state.library_repo.find_by_id(&book.library_id).await? else {
        return Ok(());
    };

    let config: crate::db::models::ScraperConfig = library
        .scraper_config
        .as_ref()
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();

    if !config.nfo_writing_enabled && !config.metadata_writing_enabled {
        return Ok(());
    }

    let target_dir = if library.library_type == "webdav" {
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        hasher.update(book.path.as_bytes());
        let book_hash = format!("{:x}", hasher.finalize());
        let temp_book_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("temp")
            .join(&book_hash);
        if !temp_book_dir.exists() {
            std::fs::create_dir_all(&temp_book_dir).ok();
        }
        temp_book_dir
    } else {
        std::path::PathBuf::from(&book.path)
    };

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
            metadata.tags.items = tags_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        metadata.touch();

        if let Err(e) = state
            .nfo_manager
            .write_book_nfo_to_dir(&target_dir, &metadata)
        {
            tracing::warn!(
                "涓轰功绫?{} 鍐欏叆 NFO 澶辫触: {}",
                book.title.as_deref().unwrap_or("?"),
                e
            );
        }
    }

    if config.metadata_writing_enabled {
        let mut metadata_json = crate::core::metadata_writer::read_metadata_json(&target_dir)
            .unwrap_or(None)
            .unwrap_or_default();

        metadata_json.title = book.title.clone();
        metadata_json.authors = book.author.clone().map(|s| vec![s]).unwrap_or_default();
        metadata_json.narrators = book.narrator.clone().map(|s| vec![s]).unwrap_or_default();
        metadata_json.description = book.description.clone();
        metadata_json.genres = book
            .genre
            .clone()
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        metadata_json.tags = book
            .tags
            .clone()
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        metadata_json.published_year = book.year.map(|y| y.to_string());

        let chapter_repo = ChapterRepository::new(state.book_repo.db().clone());
        if let Ok(chapters) = chapter_repo.find_by_book(&book.id).await {
            let mut sorted_chapters = chapters;
            sorted_chapters.sort_by(|a, b| {
                a.chapter_index
                    .unwrap_or(0)
                    .cmp(&b.chapter_index.unwrap_or(0))
                    .then_with(|| {
                        natord::compare(
                            a.title.as_deref().unwrap_or(""),
                            b.title.as_deref().unwrap_or(""),
                        )
                    })
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

        let series_list = state
            .series_repo
            .find_series_by_book(&book.id)
            .await
            .unwrap_or_default();
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

        if let Err(e) =
            crate::core::metadata_writer::write_metadata_json(&target_dir, &metadata_json)
        {
            tracing::error!(target: "audit::metadata", "涓轰功绫?{} 鍐欏叆 metadata.json 澶辫触: {}", book.title.as_deref().unwrap_or("?"), e);
        }
    }

    Ok(())
}

async fn sync_scrape_extended_metadata(
    state: &AppState,
    book: &crate::db::models::Book,
    extended: &SelectedScrapeExtendedMetadata,
) -> Result<()> {
    let Some(library) = state.library_repo.find_by_id(&book.library_id).await? else {
        return Ok(());
    };

    let config: crate::db::models::ScraperConfig = library
        .scraper_config
        .as_ref()
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();

    if !config.nfo_writing_enabled && !config.metadata_writing_enabled {
        return Ok(());
    }

    let target_dir = if library.library_type == "webdav" {
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        hasher.update(book.path.as_bytes());
        let book_hash = format!("{:x}", hasher.finalize());
        let temp_book_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("temp")
            .join(&book_hash);
        if !temp_book_dir.exists() {
            std::fs::create_dir_all(&temp_book_dir).ok();
        }
        temp_book_dir
    } else {
        std::path::PathBuf::from(&book.path)
    };

    if config.nfo_writing_enabled {
        let mut metadata = BookMetadata::new(
            book.title.clone().unwrap_or_default(),
            "ting-reader".to_string(),
            book.id.clone(),
            0,
        );
        metadata.author = book.author.clone();
        metadata.narrator = book.narrator.clone();
        metadata.subtitle = extended.subtitle.clone();
        metadata.intro = book.description.clone();
        metadata.cover_url = book.cover_url.clone();
        metadata.total_duration = extended.duration;
        if let Some(tags_str) = &book.tags {
            metadata.tags.items = tags_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Some(genre) = &book.genre {
            metadata.genre.items = genre
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        metadata.touch();

        if let Err(e) = state
            .nfo_manager
            .write_book_nfo_to_dir(&target_dir, &metadata)
        {
            tracing::warn!(
                "为书籍 {} 写入扩展 NFO 失败: {}",
                book.title.as_deref().unwrap_or("?"),
                e
            );
        }
    }

    if config.metadata_writing_enabled {
        let mut metadata_json = crate::core::metadata_writer::read_metadata_json(&target_dir)
            .unwrap_or(None)
            .unwrap_or_default();

        if extended.subtitle.is_some() {
            metadata_json.subtitle = extended.subtitle.clone();
        }
        if extended.published_year.is_some() {
            metadata_json.published_year = extended.published_year.clone();
        }
        if extended.published_date.is_some() {
            metadata_json.published_date = extended.published_date.clone();
        }
        if extended.publisher.is_some() {
            metadata_json.publisher = extended.publisher.clone();
        }
        if extended.isbn.is_some() {
            metadata_json.isbn = extended.isbn.clone();
        }
        if extended.asin.is_some() {
            metadata_json.asin = extended.asin.clone();
        }
        if extended.language.is_some() {
            metadata_json.language = extended.language.clone();
        }
        if let Some(explicit) = extended.explicit {
            metadata_json.explicit = explicit;
        }
        if let Some(abridged) = extended.abridged {
            metadata_json.abridged = abridged;
        }

        if let Err(e) =
            crate::core::metadata_writer::write_metadata_json(&target_dir, &metadata_json)
        {
            tracing::error!(target: "audit::metadata", "为书籍 {} 写入扩展 metadata.json 失败: {}", book.title.as_deref().unwrap_or("?"), e);
        }
    }

    Ok(())
}
