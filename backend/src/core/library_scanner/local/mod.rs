mod chapters;
mod metadata;

use super::{LibraryScanner, MetadataSource, ScanResult, ScanStatus};
use crate::core::error::Result;
use crate::core::library_scanner::shared::{
    infer_series_directories, parse_chapter_range_dir_name, select_mergeable_range_groups,
    ChapterRangeDir, CoalescedRangeDirs, SeriesDirectoryCandidate,
};
use crate::core::nfo_manager::BookMetadata;
use crate::db::repository::Repository;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

impl LibraryScanner {
    /// Scan a local library
    pub(crate) async fn scan_local_library(
        &self,
        library_id: &str,
        path: &Path,
        task_id: Option<&str>,
        last_scanned: Option<chrono::DateTime<chrono::Utc>>,
        scraper_config: &crate::db::models::ScraperConfig,
    ) -> Result<ScanResult> {
        let mut scan_result = ScanResult::default();
        scan_result.start_time = Some(std::time::Instant::now());

        self.update_progress_key(task_id, "scan.local.scanning", serde_json::json!({}))
            .await;

        // Get all supported extensions dynamically
        let supported_extensions = self.get_supported_extensions().await;

        // 1. Recursively find all audio files and group them by directory
        let mut dir_groups: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

        let mut walk_errors = 0usize;
        for entry in WalkDir::new(path).follow_links(true).into_iter() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    walk_errors += 1;
                    let error_path = e
                        .path()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| path.display().to_string());
                    warn!(
                        path = %error_path,
                        error = %e,
                        "Failed to read local library path during scan"
                    );
                    if scan_result.errors.len() < 20 {
                        scan_result
                            .errors
                            .push(format!("Failed to read {}: {}", error_path, e));
                    }
                    continue;
                }
            };
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if supported_extensions.contains(&ext_str) {
                        if let Some(parent) = entry_path.parent() {
                            dir_groups
                                .entry(parent.to_path_buf())
                                .or_default()
                                .push(entry_path.to_path_buf());
                        }
                    }
                }
            }
        }
        scan_result.failed_count += walk_errors;

        self.update_progress_key(
            task_id,
            "scan.audio_dirs.found",
            serde_json::json!({ "count": dir_groups.len() }),
        )
        .await;

        // 2. Process each directory group as a book
        let (dir_groups, coalesced_range_dirs) =
            coalesce_local_range_directory_groups(path, dir_groups);
        let inferred_series = infer_local_series_directories(path, dir_groups.keys());
        let total_groups = dir_groups.len();
        let mut processed_count = 0;

        // Pre-fetch all books (minimal) for the library to handle deletions and fast lookup
        // Returns: (id, path, hash, manual_corrected, match_pattern)
        let all_books_minimal = self
            .book_repo
            .find_all_minimal_by_library(library_id)
            .await
            .unwrap_or_default();

        // Build lookup maps
        // Map: Path -> (id, manual_corrected, match_pattern)
        let mut book_path_map: HashMap<PathBuf, (String, i32, Option<String>)> = HashMap::new();
        let mut book_hash_map: HashMap<String, (String, i32, Option<String>)> = HashMap::new();

        for (id, path, hash, manual_corrected, match_pattern) in &all_books_minimal {
            book_path_map.insert(
                PathBuf::from(path),
                (id.clone(), *manual_corrected, match_pattern.clone()),
            );
            book_hash_map.insert(
                hash.clone(),
                (id.clone(), *manual_corrected, match_pattern.clone()),
            );
        }

        let manual_corrected_patterns: Vec<(String, String)> = all_books_minimal
            .iter()
            .filter(|(_, _, _, mc, mp)| *mc == 1 && mp.is_some())
            .map(|(id, _, _, _, mp)| (id.clone(), mp.clone().unwrap()))
            .collect();

        let mut found_book_ids: HashSet<String> = HashSet::new();
        let mut absorbed_range_book_ids: HashMap<String, String> = HashMap::new();

        for (dir, mut files) in dir_groups {
            // Check cancellation
            self.check_cancellation(task_id).await?;

            processed_count += 1;
            let dir_name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown");

            self.update_progress_key(
                task_id,
                "scan.item.processing",
                serde_json::json!({
                    "current": processed_count,
                    "total": total_groups,
                    "name": dir_name,
                }),
            )
            .await;

            // Sort files by filename using natural sort order (e.g. 1, 2, 10 instead of 1, 10, 2)
            files.sort_by(|a, b| {
                natord::compare(a.to_string_lossy().as_ref(), b.to_string_lossy().as_ref())
            });

            // Optimization: Find existing book to avoid DB lookup
            let mut existing_info = book_path_map.get(&dir).cloned();

            // If not found by path, try hash (for moved books)
            if existing_info.is_none() {
                let book_hash = self.generate_book_hash(&dir);
                existing_info = book_hash_map.get(&book_hash).cloned();
            }

            if existing_info.is_none() {
                if let Some(child_dirs) = coalesced_range_dirs.get(&dir) {
                    for child_dir in &child_dirs.child_dirs {
                        if let Some(info) = book_path_map.get(child_dir).cloned() {
                            existing_info = Some(info);
                            break;
                        }
                    }
                }
            }

            match self
                .process_book_directory(
                    library_id,
                    &dir,
                    &files,
                    last_scanned,
                    task_id,
                    scraper_config,
                    &manual_corrected_patterns,
                    existing_info,
                    coalesced_range_dirs
                        .get(&dir)
                        .and_then(|range_dirs| range_dirs.title_override.as_deref()),
                )
                .await
            {
                Ok((book_id, status)) => {
                    scan_result.total_books += 1;
                    match status {
                        ScanStatus::Created => scan_result.books_created += 1,
                        ScanStatus::Updated => scan_result.books_updated += 1,
                        ScanStatus::Skipped => scan_result.books_skipped += 1,
                    }
                    found_book_ids.insert(book_id.clone());
                    if let Some(series_info) = inferred_series.get(&dir) {
                        if let Err(e) = self
                            .link_book_to_inferred_series(library_id, &book_id, series_info)
                            .await
                        {
                            warn!(
                                path = ?dir,
                                book_id = %book_id,
                                error = %e,
                                "Failed to link book to inferred series"
                            );
                        }
                    }
                    if let Some(child_dirs) = coalesced_range_dirs.get(&dir) {
                        for child_dir in &child_dirs.child_dirs {
                            if let Some((child_book_id, manual_corrected, _)) =
                                book_path_map.get(child_dir)
                            {
                                if child_book_id != &book_id && *manual_corrected == 0 {
                                    absorbed_range_book_ids
                                        .insert(child_book_id.clone(), book_id.clone());
                                }
                            }
                        }
                    }
                    debug!(book_id = %book_id, path = ?dir, status = ?status, "Processed book directory");
                }
                Err(e) => {
                    scan_result.failed_count += 1;
                    warn!(path = ?dir, error = %e, "Failed to process book directory");
                    scan_result
                        .errors
                        .push(format!("Failed to process {}: {}", dir.display(), e));
                }
            }

            // Periodic garbage collection to prevent memory buildup during large scans
            // Force GC after every directory to help debug memory issues with native plugins
            self.plugin_manager.garbage_collect_all().await;
        }

        for (source_id, target_id) in absorbed_range_book_ids {
            if found_book_ids.contains(&source_id) {
                continue;
            }

            if let Some(merge_service) = &self.merge_service {
                if let Err(e) = merge_service
                    .absorb_scanned_book(&target_id, &source_id)
                    .await
                {
                    warn!(
                        "Failed to absorb range-segment book {} into {}: {}",
                        source_id, target_id, e
                    );
                } else {
                    scan_result.books_deleted += 1;
                }
            } else {
                info!(
                    "Deleting range-segment book record after merging into parent book: {}",
                    source_id
                );
                if let Err(e) = self.book_repo.delete(&source_id).await {
                    warn!(
                        "Failed to delete absorbed range-segment book {}: {}",
                        source_id, e
                    );
                } else {
                    scan_result.books_deleted += 1;
                    if let Err(e) = self.chapter_repo.delete_by_book(&source_id).await {
                        warn!(
                            "Failed to delete chapters for absorbed range-segment book {}: {}",
                            source_id, e
                        );
                    }
                }
            }
        }

        // 3. Handle Deletions: Delete books that were not found in the scan and path does not exist
        for (id, path_str, _, _, _) in all_books_minimal {
            if !found_book_ids.contains(&id) {
                let path = Path::new(&path_str);
                if !path.exists() {
                    info!("Book path missing, deleting record: {}", path_str);
                    if let Err(e) = self.book_repo.delete(&id).await {
                        warn!("Failed to delete missing book {}: {}", id, e);
                    } else {
                        scan_result.books_deleted += 1;
                        if let Err(e) = self.chapter_repo.delete_by_book(&id).await {
                            warn!("Failed to delete chapters for missing book {}: {}", id, e);
                        }
                    }
                }
            }
        }

        // Final garbage collection after scan
        self.plugin_manager.garbage_collect_all().await;

        Ok(scan_result)
    }

    /// Process a directory containing audio files as a book
    pub(crate) async fn process_book_directory(
        &self,
        library_id: &str,
        dir: &Path,
        files: &[PathBuf],
        last_scanned: Option<chrono::DateTime<chrono::Utc>>,
        task_id: Option<&str>,
        scraper_config: &crate::db::models::ScraperConfig,
        manual_corrected_patterns: &[(String, String)],
        existing_info: Option<(String, i32, Option<String>)>,
        fallback_title_override: Option<&str>,
    ) -> Result<(String, ScanStatus)> {
        // Log scraper config for debugging
        debug!(
            "Processing book dir: {:?}, nfo_enabled: {}, json_enabled: {}",
            dir, scraper_config.nfo_writing_enabled, scraper_config.metadata_writing_enabled
        );

        // 0. Check New Chapter Protection (Manual Correction)
        for (book_id, pattern) in manual_corrected_patterns {
            if !pattern.is_empty() {
                let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(dir_name) {
                        info!(
                            "New Chapter Protection: Merging {} into existing book {}",
                            dir_name, book_id
                        );
                        let has_changes = self
                            .process_chapters(
                                book_id,
                                files,
                                last_scanned,
                                task_id,
                                scraper_config.use_filename_as_title,
                                scraper_config.cloud_mode,
                                None,
                                None,
                                None,
                            )
                            .await?;
                        return Ok((
                            book_id.clone(),
                            if has_changes {
                                ScanStatus::Updated
                            } else {
                                ScanStatus::Skipped
                            },
                        ));
                    }
                }
            }
        }

        // 1. Check if Book Exists
        let mut existing_book_id = None;
        let mut is_manual_corrected = false;

        let book_hash = self.generate_book_hash(dir);

        if let Some((id, mc, _)) = existing_info {
            existing_book_id = Some(id);
            is_manual_corrected = mc == 1;
        } else if let Ok(Some(book)) = self.book_repo.find_by_hash(&book_hash).await {
            existing_book_id = Some(book.id.clone());
            is_manual_corrected = book.manual_corrected == 1;
        }

        // 2. Optimization: Skip metadata update if files haven't changed
        // But do not skip if manual_corrected is false and we want to try scraping
        let max_mtime = files
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok().and_then(|m| m.modified().ok()))
            .max();
        let max_mtime_utc = max_mtime.map(|t| chrono::DateTime::<chrono::Utc>::from(t));

        let mut skip_metadata_update = false;
        if let (Some(last_scan), Some(max_mt)) = (last_scanned, max_mtime_utc) {
            // Only skip metadata update if the files haven't changed AND the book is already in the database
            if max_mt <= last_scan && existing_book_id.is_some() {
                skip_metadata_update = true;
            }
        }

        // Even if files haven't changed, if we are configured to write nfo/json, we might want to ensure they exist
        // But for pure scanning speed, we currently skip.
        // To fix the issue where scrape results aren't applied or written if files haven't changed:
        // We will NOT skip metadata update if it's NOT manual corrected, to allow new scraper results to apply.
        if skip_metadata_update && existing_book_id.is_some() && is_manual_corrected {
            let book_id = existing_book_id.unwrap();
            // Just process chapters (which also has skip logic)
            let has_changes = self
                .process_chapters(
                    &book_id,
                    files,
                    last_scanned,
                    task_id,
                    scraper_config.use_filename_as_title,
                    scraper_config.cloud_mode,
                    None,
                    None,
                    None,
                )
                .await?;

            // Check if we need to restore missing NFO/JSON files or update due to chapter changes
            if scraper_config.nfo_writing_enabled {
                let nfo_path = dir.join("book.nfo");
                if has_changes || !nfo_path.exists() {
                    if let Ok(Some(book)) = self.book_repo.find_by_id(&book_id).await {
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
                        let _ = self.nfo_manager.write_book_nfo_to_dir(dir, &metadata);
                    }
                }
            }

            if scraper_config.metadata_writing_enabled {
                let json_path = dir.join("metadata.json");
                if has_changes || !json_path.exists() {
                    if let Ok(Some(book)) = self.book_repo.find_by_id(&book_id).await {
                        // Write full metadata.json
                        let chapters = self
                            .chapter_repo
                            .find_by_book(&book_id)
                            .await
                            .unwrap_or_default();
                        let mut sorted_chapters = chapters;
                        sorted_chapters.sort_by(|a, b| {
                            a.chapter_index
                                .unwrap_or(0)
                                .cmp(&b.chapter_index.unwrap_or(0))
                        });
                        let mut abs_chapters = Vec::new();
                        let mut current_time = 0.0;
                        for (idx, ch) in sorted_chapters.iter().enumerate() {
                            let duration = ch.duration.unwrap_or(0) as f64;
                            abs_chapters.push(
                                crate::core::metadata_writer::AudiobookshelfChapter {
                                    id: idx as u32,
                                    start: current_time,
                                    end: current_time + duration,
                                    title: ch.title.clone().unwrap_or_default(),
                                },
                            );
                            current_time += duration;
                        }
                        let extended_meta =
                            crate::core::metadata_writer::ExtendedMetadata::default();
                        let series_list = self
                            .series_repo
                            .find_series_by_book(&book_id)
                            .await
                            .unwrap_or_default();
                        let mut series_titles = Vec::new();
                        for series in series_list {
                            let formatted_title = if let Ok(books) =
                                self.series_repo.find_books_by_series(&series.id).await
                            {
                                if let Some((_, order)) =
                                    books.iter().find(|(b, _)| b.id == book_id)
                                {
                                    format!("{} #{}", series.title, order)
                                } else {
                                    series.title.clone()
                                }
                            } else {
                                series.title.clone()
                            };

                            if !series_titles.contains(&formatted_title) {
                                series_titles.push(formatted_title);
                            }
                        }
                        let metadata_json =
                            crate::core::metadata_writer::AudiobookshelfMetadata::new(
                                &book,
                                abs_chapters,
                                extended_meta,
                                series_titles,
                            );
                        let _ =
                            crate::core::metadata_writer::write_metadata_json(dir, &metadata_json);
                    }
                }
            }

            return Ok((
                book_id,
                if has_changes {
                    ScanStatus::Updated
                } else {
                    ScanStatus::Skipped
                },
            ));
        }

        // 3. Extract Metadata
        let (scanned_meta, source) = self
            .extract_final_metadata(dir, files, scraper_config, fallback_title_override)
            .await;

        let mut title = scanned_meta.title.unwrap_or_else(|| {
            fallback_title_override
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("Unknown Book")
                .to_string()
        });
        if let Some(fallback_title) = fallback_title_override {
            if !fallback_title.trim().is_empty() && source == MetadataSource::Fallback {
                title = fallback_title.to_string();
            }
        }
        let mut author = scanned_meta.author;
        let mut narrator = scanned_meta.narrator;
        let mut description = scanned_meta.description;
        let mut tags = scanned_meta.tags;
        let mut genre = scanned_meta.genre;
        let mut cover_url = scanned_meta.cover_url;

        // Extended fields
        let subtitle = scanned_meta.subtitle;
        let published_year = scanned_meta.published_year;
        let published_date = scanned_meta.published_date;
        let publisher = scanned_meta.publisher;
        let isbn = scanned_meta.isbn;
        let asin = scanned_meta.asin;
        let language = scanned_meta.language;
        let explicit = scanned_meta.explicit;
        let abridged = scanned_meta.abridged;
        let json_tags = scanned_meta.json_tags;
        let json_series = scanned_meta.json_series;
        let json_chapters = scanned_meta.json_chapters;
        let chapter_title_template = scanned_meta.chapter_title_template;
        let chapter_titles = scanned_meta.chapter_titles;

        if author.is_none() {
            author = Some("Unknown".to_string());
        }

        // 3. Apply Manual Correction or Existing Data
        if is_manual_corrected {
            if let Some(id) = &existing_book_id {
                if let Ok(Some(book)) = self.book_repo.find_by_id(id).await {
                    // Use existing values if present, otherwise fall back to extracted
                    title = book.title.unwrap_or(title);
                    if book.author.is_some() {
                        author = book.author;
                    }
                    if book.narrator.is_some() {
                        narrator = book.narrator;
                    }
                    if book.description.is_some() {
                        description = book.description;
                    }
                    if book.tags.is_some() {
                        tags = book.tags;
                    }
                    if book.genre.is_some() {
                        genre = book.genre;
                    }
                    if book.cover_url.is_some() {
                        cover_url = book.cover_url;
                    }
                    // theme_color will be recalculated if cover_url changed later
                }
            }
        }

        // Theme Color
        let mut theme_color = None;
        if let Some(ref url) = cover_url {
            let cover_path = if url.starts_with("http") || url.starts_with("//") {
                url.clone()
            } else {
                let p = Path::new(url);
                if p.exists() {
                    url.clone()
                } else {
                    dir.join(url).to_string_lossy().to_string()
                }
            };

            // For local paths, we need to handle Windows UNC paths carefully
            let normalized_path =
                if !cover_path.starts_with("http") && !cover_path.starts_with("//") {
                    let p = Path::new(&cover_path);
                    // First try to canonicalize to resolve relative paths
                    let mut path_str = p
                        .canonicalize()
                        .unwrap_or_else(|_| p.to_path_buf())
                        .to_string_lossy()
                        .to_string();

                    // Then strip Windows UNC prefix if present, and normalize slashes
                    if path_str.starts_with("\\\\?\\") || path_str.starts_with("//?/") {
                        path_str = path_str[4..].to_string();
                    }
                    path_str.replace('\\', "/")
                } else {
                    cover_path
                };

            if let Ok(Some(color)) = crate::core::color::calculate_theme_color_with_client(
                &normalized_path,
                &self.http_client,
            )
            .await
            {
                theme_color = Some(color);
            }
        }

        // 4. Create/Update Book
        let book_id = existing_book_id.unwrap_or_else(|| Uuid::new_v4().to_string());

        let mut book = crate::db::models::Book {
            id: book_id.clone(),
            library_id: library_id.to_string(),
            title: Some(title.clone()),
            author: author.clone(),
            narrator: narrator.clone(),
            cover_url: cover_url.clone(),
            description: description.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            path: dir.to_string_lossy().to_string(),
            hash: book_hash.clone(),
            theme_color: theme_color.clone(),
            skip_intro: 0,
            skip_outro: 0,
            tags: tags.clone(),
            genre: genre.clone(),
            year: published_year.as_ref().and_then(|y| y.parse::<i32>().ok()),
            manual_corrected: if is_manual_corrected { 1 } else { 0 },
            match_pattern: None,
            chapter_regex: None,
        };

        let status = if let Ok(Some(existing)) = self.book_repo.find_by_id(&book_id).await {
            if existing.manual_corrected == 0 {
                // Preserve chapter_regex from existing book if not set in metadata
                if book.chapter_regex.is_none() && existing.chapter_regex.is_some() {
                    book.chapter_regex = existing.chapter_regex;
                }
                self.book_repo.update(&book).await?;
                ScanStatus::Updated
            } else {
                ScanStatus::Skipped
            }
        } else {
            self.book_repo.create(&book).await?;
            ScanStatus::Created
        };

        // 5. Process Chapters
        let chapters_changed = self
            .process_chapters(
                &book_id,
                files,
                last_scanned,
                task_id,
                scraper_config.use_filename_as_title,
                scraper_config.cloud_mode,
                json_chapters,
                chapter_title_template.as_deref(),
                if chapter_titles.is_empty() {
                    None
                } else {
                    Some(chapter_titles.as_slice())
                },
            )
            .await?;

        // 5.1 Process Series
        if !json_series.is_empty() {
            for series_title_raw in json_series {
                let series_title_raw = series_title_raw.trim();
                if series_title_raw.is_empty() {
                    continue;
                }

                // Parse series title and optional sequence number
                let mut series_title = series_title_raw.to_string();
                let mut explicit_order = None;

                if let Some(idx) = series_title_raw.rfind(" #") {
                    let (name_part, num_part) = series_title_raw.split_at(idx);
                    let num_str = num_part[2..].trim();
                    if let Ok(order) = num_str.parse::<i32>() {
                        series_title = name_part.trim().to_string();
                        explicit_order = Some(order);
                    }
                }

                // Find or create series atomically (globally across all libraries to handle concurrent syncs and multiple libraries)
                let new_series = crate::db::models::Series {
                    id: Uuid::new_v4().to_string(),
                    library_id: library_id.to_string(),
                    title: series_title.clone(),
                    author: author.clone(), // Initial author from first found book
                    narrator: narrator.clone(),
                    cover_url: cover_url.clone(),
                    description: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };
                let series = self.series_repo.find_or_create_by_title(new_series).await?;

                // Link book to series if not already linked
                let books = self.series_repo.find_books_by_series(&series.id).await?;
                if let Some((_, current_order)) = books.iter().find(|(b, _)| b.id == book_id) {
                    // Already linked, update order if explicit order changed
                    if let Some(o) = explicit_order {
                        if *current_order != o {
                            self.series_repo
                                .add_book(crate::db::models::SeriesBook {
                                    series_id: series.id.clone(),
                                    book_id: book_id.clone(),
                                    book_order: o,
                                })
                                .await?;
                        }
                    }
                } else {
                    // Not linked, insert it
                    let order = if let Some(o) = explicit_order {
                        o
                    } else {
                        books.len() as i32 + 1
                    };

                    self.series_repo
                        .add_book(crate::db::models::SeriesBook {
                            series_id: series.id.clone(),
                            book_id: book_id.clone(),
                            book_order: order,
                        })
                        .await?;

                    // If no explicit order, resort all books in series by natural order of title
                    if explicit_order.is_none() {
                        let mut all_books =
                            self.series_repo.find_books_by_series(&series.id).await?;
                        all_books.sort_by(|a, b| {
                            let t1 = a.0.title.as_deref().unwrap_or("");
                            let t2 = b.0.title.as_deref().unwrap_or("");
                            natord::compare(t1, t2)
                        });

                        let new_orders: Vec<(String, i32)> = all_books
                            .into_iter()
                            .enumerate()
                            .map(|(i, (b, _))| (b.id, (i + 1) as i32))
                            .collect();

                        self.series_repo
                            .update_book_orders(&series.id, new_orders)
                            .await?;
                    }

                    // DO NOT update series metadata based on subsequent books to avoid instability
                    // Series metadata should only be set on creation or manual update
                }
            }
        }

        // 6. Write NFO/Metadata
        if scraper_config.nfo_writing_enabled {
            debug!("Writing NFO for book: {}", book_id);
            if let Ok(Some(book)) = self.book_repo.find_by_id(&book_id).await {
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
                if let Err(e) = self
                    .nfo_manager
                    .write_book_nfo_to_dir(Path::new(&book.path), &metadata)
                {
                    warn!("Failed to write NFO: {}", e);
                } else {
                    info!("Successfully wrote NFO to: {}", book.path);
                }
            }
        }

        if scraper_config.metadata_writing_enabled {
            debug!("Writing metadata.json for book: {}", book_id);
            let chapters = self.chapter_repo.find_by_book(&book_id).await?;
            let mut sorted_chapters = chapters;
            sorted_chapters.sort_by(|a, b| {
                a.chapter_index
                    .unwrap_or(0)
                    .cmp(&b.chapter_index.unwrap_or(0))
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
            let extended_meta = crate::core::metadata_writer::ExtendedMetadata {
                subtitle,
                published_year,
                published_date,
                publisher,
                isbn,
                asin,
                language,
                explicit,
                abridged,
                tags: json_tags,
            };

            // Get series for this book
            let series_list = self
                .series_repo
                .find_series_by_book(&book_id)
                .await
                .unwrap_or_default();
            let mut series_titles = Vec::new();
            for series in series_list {
                let formatted_title =
                    if let Ok(books) = self.series_repo.find_books_by_series(&series.id).await {
                        if let Some((_, order)) = books.iter().find(|(b, _)| b.id == book_id) {
                            format!("{} #{}", series.title, order)
                        } else {
                            series.title.clone()
                        }
                    } else {
                        series.title.clone()
                    };

                // Prevent duplicates
                if !series_titles.contains(&formatted_title) {
                    series_titles.push(formatted_title);
                }
            }

            let metadata_json = crate::core::metadata_writer::AudiobookshelfMetadata::new(
                &book,
                abs_chapters,
                extended_meta,
                series_titles,
            );
            if let Err(e) = crate::core::metadata_writer::write_metadata_json(dir, &metadata_json) {
                warn!(
                    target: "audit::metadata",
                    path = %dir.display(),
                    error = %e,
                    message_key = "metadata.json.write_failed",
                    message_params = %serde_json::json!({
                        "path": dir.display().to_string(),
                        "error": e.to_string(),
                    }),
                    "Failed to write metadata.json"
                );
            } else {
                debug!("Successfully wrote metadata.json to: {:?}", dir);
            }
        }

        let final_status = match status {
            ScanStatus::Created => ScanStatus::Created,
            _ => {
                if chapters_changed {
                    ScanStatus::Updated
                } else {
                    status
                }
            }
        };

        Ok((book_id, final_status))
    }

    pub(super) fn find_cover_image(&self, dir: &Path) -> Option<String> {
        let cover_names = [
            "cover.jpg",
            "cover.png",
            "cover.jpeg",
            "folder.jpg",
            "folder.png",
        ];
        for name in cover_names {
            let path = dir.join(name);
            if path.exists() {
                // Return path with forward slashes for better JSON/URL compatibility
                return Some(path.to_string_lossy().replace('\\', "/"));
            }
        }
        if let Ok(mut entries) = std::fs::read_dir(dir) {
            while let Some(Ok(entry)) = entries.next() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if ["jpg", "jpeg", "png", "webp"].contains(&ext_str.as_str()) {
                            // Return path with forward slashes for better JSON/URL compatibility
                            return Some(path.to_string_lossy().replace('\\', "/"));
                        }
                    }
                }
            }
        }
        None
    }

    fn generate_book_hash(&self, audiobook_dir: &Path) -> String {
        let path_str = audiobook_dir.to_string_lossy();
        let mut hasher = Sha256::new();
        hasher.update(path_str.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

fn coalesce_local_range_directory_groups(
    root: &Path,
    mut dir_groups: HashMap<PathBuf, Vec<PathBuf>>,
) -> (
    HashMap<PathBuf, Vec<PathBuf>>,
    HashMap<PathBuf, CoalescedRangeDirs<PathBuf>>,
) {
    let mut candidates: HashMap<PathBuf, Vec<(PathBuf, ChapterRangeDir)>> = HashMap::new();

    for dir in dir_groups.keys() {
        let Some(parent) = dir.parent() else {
            continue;
        };
        let Some(dir_name) = dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(range_dir) = parse_chapter_range_dir_name(dir_name) else {
            continue;
        };

        candidates
            .entry(parent.to_path_buf())
            .or_default()
            .push((dir.clone(), range_dir));
    }

    let mut coalesced_range_dirs = HashMap::new();

    for (parent, entries) in candidates {
        let parent_name = parent
            .file_name()
            .and_then(|name| name.to_str())
            .or_else(|| root.file_name().and_then(|name| name.to_str()))
            .unwrap_or("");

        let ranges: Vec<ChapterRangeDir> = entries.iter().map(|(_, range)| range.clone()).collect();

        for group in select_mergeable_range_groups(parent_name, &ranges) {
            let mut selected: Vec<(PathBuf, ChapterRangeDir)> = group
                .indices
                .into_iter()
                .map(|index| entries[index].clone())
                .collect();
            selected.sort_by_key(|(_, range)| (range.start, range.end));

            let Some(first_child_dir) = selected.first().map(|(child_dir, _)| child_dir.clone())
            else {
                continue;
            };

            let target_dir = if group.merge_into_parent {
                parent.clone()
            } else {
                first_child_dir
            };

            let child_dirs: Vec<PathBuf> = selected
                .iter()
                .map(|(child_dir, _)| child_dir.clone())
                .collect();

            for child_dir in &child_dirs {
                if let Some(mut child_files) = dir_groups.remove(child_dir) {
                    dir_groups
                        .entry(target_dir.clone())
                        .or_default()
                        .append(&mut child_files);
                }
            }

            if !child_dirs.is_empty() {
                coalesced_range_dirs.insert(
                    target_dir,
                    CoalescedRangeDirs {
                        child_dirs,
                        title_override: group.title,
                    },
                );
            }
        }
    }

    (dir_groups, coalesced_range_dirs)
}

fn infer_local_series_directories<'a>(
    root: &Path,
    dirs: impl Iterator<Item = &'a PathBuf>,
) -> HashMap<PathBuf, crate::core::library_scanner::shared::InferredSeriesInfo> {
    let candidates: Vec<SeriesDirectoryCandidate<PathBuf>> = dirs
        .filter_map(|dir| {
            let parent = dir.parent()?;
            let name = dir.file_name()?.to_str()?;
            let parent_name = parent
                .file_name()
                .and_then(|value| value.to_str())
                .or_else(|| root.file_name().and_then(|value| value.to_str()))
                .unwrap_or("");

            Some(SeriesDirectoryCandidate {
                key: dir.clone(),
                parent_key: parent.to_string_lossy().to_string(),
                parent_name: parent_name.to_string(),
                name: name.to_string(),
            })
        })
        .collect();

    infer_series_directories(&candidates)
}
