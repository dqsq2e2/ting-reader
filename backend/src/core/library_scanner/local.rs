use super::{LibraryScanner, ScanResult, ScanStatus, MetadataSource, STANDARD_EXTENSIONS};
use crate::core::error::{Result, TingError};
use crate::db::models::Chapter;
use crate::core::nfo_manager::BookMetadata;
use crate::db::repository::Repository;
use crate::plugin::manager::FormatMethod;
use crate::plugin::types::PluginType;
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use walkdir::WalkDir;
use tracing::{debug, info, warn};
use uuid::Uuid;
use sha2::{Digest, Sha256};
use std::io::Read;

#[derive(Debug, Default, Clone)]
struct ScannedMetadata {
    title: Option<String>,
    author: Option<String>,
    narrator: Option<String>,
    description: Option<String>,
    tags: Option<String>,
    genre: Option<String>,
    cover_url: Option<String>,
    subtitle: Option<String>,
    published_year: Option<String>,
    published_date: Option<String>,
    publisher: Option<String>,
    isbn: Option<String>,
    asin: Option<String>,
    language: Option<String>,
    explicit: bool,
    abridged: bool,
    json_tags: Vec<String>,
    json_series: Vec<String>,
    json_chapters: Option<Vec<crate::core::metadata_writer::AudiobookshelfChapter>>,
}

impl ScannedMetadata {
    fn merge(&mut self, other: ScannedMetadata) {
        if let Some(t) = other.title { if !t.trim().is_empty() { self.title = Some(t); } }
        if other.author.is_some() { self.author = other.author; }
        if other.narrator.is_some() { self.narrator = other.narrator; }
        if other.description.is_some() { self.description = other.description; }
        if other.tags.is_some() { self.tags = other.tags; }
        if other.genre.is_some() { self.genre = other.genre; }
        if let Some(c) = other.cover_url { if !c.trim().is_empty() { self.cover_url = Some(c); } }
        if other.subtitle.is_some() { self.subtitle = other.subtitle; }
        if other.published_year.is_some() { self.published_year = other.published_year; }
        if other.published_date.is_some() { self.published_date = other.published_date; }
        if other.publisher.is_some() { self.publisher = other.publisher; }
        if other.isbn.is_some() { self.isbn = other.isbn; }
        if other.asin.is_some() { self.asin = other.asin; }
        if other.language.is_some() { self.language = other.language; }
        if other.explicit { self.explicit = true; }
        if other.abridged { self.abridged = true; }
        if !other.json_tags.is_empty() { self.json_tags = other.json_tags; }
        if !other.json_series.is_empty() { self.json_series = other.json_series; }
        if other.json_chapters.is_some() { self.json_chapters = other.json_chapters; }
    }
}

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
        
        self.update_progress(task_id, "正在扫描本地目录...".to_string()).await;

        // Get all supported extensions dynamically
        let supported_extensions = self.get_supported_extensions().await;
        
        // 1. Recursively find all audio files and group them by directory
        let mut dir_groups: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if supported_extensions.contains(&ext_str) {
                        if let Some(parent) = entry_path.parent() {
                            dir_groups.entry(parent.to_path_buf()).or_default().push(entry_path.to_path_buf());
                        }
                    }
                }
            }
        }

        self.update_progress(task_id, format!("找到 {} 个包含音频文件的目录", dir_groups.len())).await;

        // 2. Process each directory group as a book
        let total_groups = dir_groups.len();
        let mut processed_count = 0;

        // Pre-fetch all books (minimal) for the library to handle deletions and fast lookup
        // Returns: (id, path, hash, manual_corrected, match_pattern)
        let all_books_minimal = self.book_repo.find_all_minimal_by_library(library_id).await.unwrap_or_default();
        
        // Build lookup maps
        // Map: Path -> (id, manual_corrected, match_pattern)
        let mut book_path_map: HashMap<PathBuf, (String, i32, Option<String>)> = HashMap::new();
        let mut book_hash_map: HashMap<String, (String, i32, Option<String>)> = HashMap::new();
        
        for (id, path, hash, manual_corrected, match_pattern) in &all_books_minimal {
            book_path_map.insert(PathBuf::from(path), (id.clone(), *manual_corrected, match_pattern.clone()));
            book_hash_map.insert(hash.clone(), (id.clone(), *manual_corrected, match_pattern.clone()));
        }

        let manual_corrected_patterns: Vec<(String, String)> = all_books_minimal.iter()
            .filter(|(_, _, _, mc, mp)| *mc == 1 && mp.is_some())
            .map(|(id, _, _, _, mp)| (id.clone(), mp.clone().unwrap()))
            .collect();
            
        let mut found_book_ids: HashSet<String> = HashSet::new();

        for (dir, mut files) in dir_groups {
            // Check cancellation
            self.check_cancellation(task_id).await?;
            
            processed_count += 1;
            let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown");
            
            self.update_progress(task_id, format!("处理中 ({}/{}): {}", processed_count, total_groups, dir_name)).await;

            // Sort files by filename using natural sort order (e.g. 1, 2, 10 instead of 1, 10, 2)
            files.sort_by(|a, b| natord::compare(a.to_string_lossy().as_ref(), b.to_string_lossy().as_ref()));

            // Optimization: Find existing book to avoid DB lookup
            let mut existing_info = book_path_map.get(&dir).cloned();
            
            // If not found by path, try hash (for moved books)
            if existing_info.is_none() {
                let book_hash = self.generate_book_hash(&dir);
                existing_info = book_hash_map.get(&book_hash).cloned();
            }

            match self.process_book_directory(library_id, &dir, &files, last_scanned, task_id, scraper_config, &manual_corrected_patterns, existing_info).await {
                Ok((book_id, status)) => {
                    scan_result.total_books += 1;
                    match status {
                        ScanStatus::Created => scan_result.books_created += 1,
                        ScanStatus::Updated => scan_result.books_updated += 1,
                        ScanStatus::Skipped => scan_result.books_skipped += 1,
                    }
                    found_book_ids.insert(book_id.clone());
                    debug!(book_id = %book_id, path = ?dir, status = ?status, "Processed book directory");
                }
                Err(e) => {
                    scan_result.failed_count += 1;
                    warn!(path = ?dir, error = %e, "Failed to process book directory");
                    scan_result.errors.push(format!(
                        "Failed to process {}: {}",
                        dir.display(),
                        e
                    ));
                }
            }

            // Periodic garbage collection to prevent memory buildup during large scans
            // Force GC after every directory to help debug memory issues with native plugins
            self.plugin_manager.garbage_collect_all().await;
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
    ) -> Result<(String, ScanStatus)> {
        // Log scraper config for debugging
        debug!("Processing book dir: {:?}, nfo_enabled: {}, json_enabled: {}", 
            dir, 
            scraper_config.nfo_writing_enabled, 
            scraper_config.metadata_writing_enabled
        );

        // 0. Check New Chapter Protection (Manual Correction)
        for (book_id, pattern) in manual_corrected_patterns {
            if !pattern.is_empty() {
                let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(dir_name) {
                        info!("New Chapter Protection: Merging {} into existing book {}", dir_name, book_id);
                        let has_changes = self.process_chapters(book_id, files, last_scanned, task_id, scraper_config.use_filename_as_title, None).await?;
                        return Ok((book_id.clone(), if has_changes { ScanStatus::Updated } else { ScanStatus::Skipped }));
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
        let max_mtime = files.iter()
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
             let has_changes = self.process_chapters(&book_id, files, last_scanned, task_id, scraper_config.use_filename_as_title, None).await?;
             
             // Check if we need to restore missing NFO/JSON files or update due to chapter changes
             if scraper_config.nfo_writing_enabled {
                 let nfo_path = dir.join("book.nfo");
                 if has_changes || !nfo_path.exists() {
                     if let Ok(Some(book)) = self.book_repo.find_by_id(&book_id).await {
                         let mut metadata = BookMetadata::new(book.title.clone().unwrap_or_default(), "ting-reader".to_string(), book.id.clone(), 0);
                         metadata.author = book.author.clone();
                         metadata.narrator = book.narrator.clone();
                         metadata.intro = book.description.clone();
                         metadata.cover_url = book.cover_url.clone();
                         if let Some(tags_str) = &book.tags { metadata.tags.items = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(); }
                         let _ = self.nfo_manager.write_book_nfo_to_dir(dir, &metadata);
                     }
                 }
             }
             
             if scraper_config.metadata_writing_enabled {
                 let json_path = dir.join("metadata.json");
                 if has_changes || !json_path.exists() {
                     if let Ok(Some(book)) = self.book_repo.find_by_id(&book_id).await {
                         // Write full metadata.json
                         let chapters = self.chapter_repo.find_by_book(&book_id).await.unwrap_or_default();
                         let mut sorted_chapters = chapters;
                         sorted_chapters.sort_by(|a, b| a.chapter_index.unwrap_or(0).cmp(&b.chapter_index.unwrap_or(0)));
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
                         let extended_meta = crate::core::metadata_writer::ExtendedMetadata::default();
                         let series_list = self.series_repo.find_series_by_book(&book_id).await.unwrap_or_default();
                         let mut series_titles = Vec::new();
                         for series in series_list {
                             let formatted_title = if let Ok(books) = self.series_repo.find_books_by_series(&series.id).await {
                                 if let Some((_, order)) = books.iter().find(|(b, _)| b.id == book_id) {
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
                         let metadata_json = crate::core::metadata_writer::AudiobookshelfMetadata::new(&book, abs_chapters, extended_meta, series_titles);
                         let _ = crate::core::metadata_writer::write_metadata_json(dir, &metadata_json);
                     }
                 }
             }
             
             return Ok((book_id, if has_changes { ScanStatus::Updated } else { ScanStatus::Skipped }));
        }

        // 3. Extract Metadata
        let (scanned_meta, _source) = self.extract_final_metadata(dir, files, scraper_config).await;
        
        let mut title = scanned_meta.title.unwrap_or_else(|| "Unknown Book".to_string());
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

        if author.is_none() { author = Some("Unknown".to_string()); }

        // 3. Apply Manual Correction or Existing Data
        if is_manual_corrected {
             if let Some(id) = &existing_book_id {
                if let Ok(Some(book)) = self.book_repo.find_by_id(id).await {
                    // Use existing values if present, otherwise fall back to extracted
                    title = book.title.unwrap_or(title);
                    if book.author.is_some() { author = book.author; }
                    if book.narrator.is_some() { narrator = book.narrator; }
                    if book.description.is_some() { description = book.description; }
                    if book.tags.is_some() { tags = book.tags; }
                    if book.genre.is_some() { genre = book.genre; }
                    if book.cover_url.is_some() { cover_url = book.cover_url; }
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
            let normalized_path = if !cover_path.starts_with("http") && !cover_path.starts_with("//") {
                let p = Path::new(&cover_path);
                // First try to canonicalize to resolve relative paths
                let mut path_str = p.canonicalize().unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().to_string();
                
                // Then strip Windows UNC prefix if present, and normalize slashes
                if path_str.starts_with("\\\\?\\") || path_str.starts_with("//?/") {
                    path_str = path_str[4..].to_string();
                }
                path_str.replace('\\', "/")
            } else {
                cover_path
            };
            
            if let Ok(Some(color)) = crate::core::color::calculate_theme_color_with_client(&normalized_path, &self.http_client).await {
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
        let chapters_changed = self.process_chapters(&book_id, files, last_scanned, task_id, scraper_config.use_filename_as_title, json_chapters).await?;

        // 5.1 Process Series
        if !json_series.is_empty() {
            for series_title_raw in json_series {
                let series_title_raw = series_title_raw.trim();
                if series_title_raw.is_empty() { continue; }
                
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
                             self.series_repo.add_book(crate::db::models::SeriesBook {
                                 series_id: series.id.clone(),
                                 book_id: book_id.clone(),
                                 book_order: o,
                             }).await?;
                         }
                     }
                } else {
                     // Not linked, insert it
                     let order = if let Some(o) = explicit_order {
                         o
                     } else {
                         books.len() as i32 + 1
                     };
                     
                     self.series_repo.add_book(crate::db::models::SeriesBook {
                         series_id: series.id.clone(),
                         book_id: book_id.clone(),
                         book_order: order,
                     }).await?;

                     // If no explicit order, resort all books in series by natural order of title
                     if explicit_order.is_none() {
                         let mut all_books = self.series_repo.find_books_by_series(&series.id).await?;
                         all_books.sort_by(|a, b| {
                             let t1 = a.0.title.as_deref().unwrap_or("");
                             let t2 = b.0.title.as_deref().unwrap_or("");
                             natord::compare(t1, t2)
                         });
                         
                         let new_orders: Vec<(String, i32)> = all_books.into_iter()
                             .enumerate()
                             .map(|(i, (b, _))| (b.id, (i + 1) as i32))
                             .collect();
                             
                         self.series_repo.update_book_orders(&series.id, new_orders).await?;
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
                let mut metadata = BookMetadata::new(book.title.clone().unwrap_or_default(), "ting-reader".to_string(), book.id.clone(), 0);
                metadata.author = book.author.clone();
                metadata.narrator = book.narrator.clone();
                metadata.intro = book.description.clone();
                metadata.cover_url = book.cover_url.clone();
                if let Some(tags_str) = &book.tags { metadata.tags.items = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(); }
                if let Err(e) = self.nfo_manager.write_book_nfo_to_dir(Path::new(&book.path), &metadata) {
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
            sorted_chapters.sort_by(|a, b| a.chapter_index.unwrap_or(0).cmp(&b.chapter_index.unwrap_or(0)));
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
                subtitle, published_year, published_date, publisher, isbn, asin, language,
                explicit, abridged, tags: json_tags,
            };
            
            // Get series for this book
            let series_list = self.series_repo.find_series_by_book(&book_id).await.unwrap_or_default();
            let mut series_titles = Vec::new();
            for series in series_list {
                let formatted_title = if let Ok(books) = self.series_repo.find_books_by_series(&series.id).await {
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
            
            let metadata_json = crate::core::metadata_writer::AudiobookshelfMetadata::new(&book, abs_chapters, extended_meta, series_titles);
            if let Err(e) = crate::core::metadata_writer::write_metadata_json(dir, &metadata_json) {
                warn!(target: "audit::metadata", "写入 metadata.json 失败 (目录: {:?}): {}", dir, e);
            } else {
                debug!("Successfully wrote metadata.json to: {:?}", dir);
            }
        }

        let final_status = match status {
            ScanStatus::Created => ScanStatus::Created,
            _ => if chapters_changed { ScanStatus::Updated } else { status }
        };

        Ok((book_id, final_status))
    }

    async fn extract_final_metadata(
        &self,
        dir: &Path,
        files: &[PathBuf],
        scraper_config: &crate::db::models::ScraperConfig,
    ) -> (ScannedMetadata, MetadataSource) {
        let mut final_meta = ScannedMetadata::default();
        let mut final_source = MetadataSource::Fallback;

        // 0. Base: Directory Name
        let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown Book");
        let (cleaned_title, _) = self.text_cleaner.clean_chapter_title(dir_name, None);
        final_meta.title = Some(cleaned_title);

        // Fallback Author from "Author - Title" pattern
        if dir_name.contains(" - ") {
             let parts: Vec<&str> = dir_name.split(" - ").collect();
             if parts.len() >= 2 {
                 final_meta.author = Some(parts[0].trim().to_string());
                 // Also update title if we are assuming Author - Title pattern
                 if final_meta.title.is_some() && final_meta.title.as_deref() == Some(dir_name) {
                     let (cleaned_title_part, _) = self.text_cleaner.clean_chapter_title(parts[1], None);
                     final_meta.title = Some(cleaned_title_part);
                 }
             }
        }

        // 1. Check if we should force filename as title
        debug!("Processing dir: {:?}, scraper_config: {:?}", dir, scraper_config);

        // 2. Iterate Priority List
        let default_priority = vec![
            "scraper".to_string(), 
            "audio_metadata".to_string(),
            "local_metadata".to_string() 
        ];
        
        let priority_list = if scraper_config.metadata_priority.is_empty() {
            &default_priority
        } else {
            &scraper_config.metadata_priority
        };

        for source_type in priority_list.iter().rev() {
            match source_type.as_str() {
                "local_metadata" => {
                    // Try to find local cover image first, so it gets merged
                    if let Some(path) = self.find_cover_image(dir) {
                        final_meta.cover_url = Some(path);
                    }
                    
                    if let Some(meta) = self.extract_from_nfo(dir) {
                        if scraper_config.use_filename_as_title {
                            let mut m = meta.clone();
                            m.title = None;
                            final_meta.merge(m);
                        } else {
                            final_meta.merge(meta);
                            if final_meta.title.is_some() {
                                final_source = MetadataSource::Nfo;
                            }
                        }
                    }
                    if let Some(meta) = self.extract_from_json(dir) {
                        if scraper_config.use_filename_as_title {
                            let mut m = meta.clone();
                            m.title = None;
                            final_meta.merge(m);
                        } else {
                            final_meta.merge(meta);
                            if final_meta.title.is_some() {
                                final_source = MetadataSource::Nfo;
                            }
                        }
                    }
                },
                "audio_metadata" => {
                    if let Some(meta) = self.extract_from_audio(dir, files, scraper_config.extract_audio_cover).await {
                        if scraper_config.use_filename_as_title {
                            let mut m = meta.clone();
                            m.title = None;
                            final_meta.merge(m);
                        } else {
                            // Bugfix: If the audio metadata title is empty, or if we want to preserve the folder title as fallback,
                            // we should still keep the extracted cover. The issue was that a bad title from audio metadata
                            // was completely overwriting everything else.
                            
                            // Only merge title if it's considered valid (not empty and not a generic track name)
                            let mut m = meta.clone();
                            if let Some(ref title) = m.title {
                                let lower_title = title.to_lowercase();
                                if lower_title.starts_with("track") || lower_title.trim().is_empty() {
                                    m.title = None; // Ignore bad embedded titles
                                }
                            }
                            
                            final_meta.merge(m);
                            if final_meta.title.is_some() {
                                final_source = MetadataSource::FileMetadata;
                            }
                        }
                    }
                },
                "scraper" => {
                    if let Some(ref title) = final_meta.title {
                         if let Some(meta) = self.extract_from_scraper(title, &final_meta.author, scraper_config).await {
                             if scraper_config.use_filename_as_title {
                                 let mut m = meta.clone();
                                 m.title = None;
                                 final_meta.merge(m);
                             } else {
                                 final_meta.merge(meta);
                             }
                         }
                    }
                },
                _ => {}
            }
        }
        
        // 2. Post-processing: Cover Image
        // If no cover URL yet, try finding local file (cover.jpg)
        if final_meta.cover_url.is_none() {
             if let Some(path) = self.find_cover_image(dir) {
                 final_meta.cover_url = Some(path);
             }
        }
        
        // 3. Fallback Cover Extraction (if still no cover, extract from ID3 and save)
        // This runs only if cover is still missing, regardless of priority, 
        // because if "audio_metadata" was high priority, it would have set cover_url from plugin/id3.
        if scraper_config.extract_audio_cover && final_meta.cover_url.is_none() && !files.is_empty() {
             let first_file = &files[0];
             // We've already tried extracting cover in extract_from_audio above for both standard and non-standard.
             // This is a final fallback just in case the file wasn't picked up by the priority system.
             if let Some(path) = self.extract_and_save_cover(first_file, dir) {
                 final_meta.cover_url = Some(path);
             } else {
                 // Try extracting cover from non-standard files (like .xm) via plugin
                 if let Some(meta) = self.extract_from_audio(dir, files, true).await {
                     if meta.cover_url.is_some() {
                         final_meta.cover_url = meta.cover_url;
                     }
                 }
             }
        }

        // 4. Validate local cover paths to ensure they exist
        if let Some(ref url) = final_meta.cover_url {
            if !url.starts_with("http") && !url.starts_with("//") {
                let p = Path::new(url);
                if !p.exists() {
                    let rel_p = dir.join(url);
                    if !rel_p.exists() {
                        final_meta.cover_url = None;
                    } else {
                        final_meta.cover_url = Some(rel_p.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }

        (final_meta, final_source)
    }

    fn extract_from_nfo(&self, dir: &Path) -> Option<ScannedMetadata> {
        let nfo_path = dir.join("book.nfo");
        if let Ok(meta) = self.nfo_manager.read_book_nfo(&nfo_path) {
            return Some(ScannedMetadata {
                title: if meta.title.is_empty() { None } else { Some(meta.title) },
                author: meta.author,
                narrator: meta.narrator,
                description: meta.intro,
                tags: Some(meta.tags.items.join(",")),
                genre: Some(meta.genre.items.join(",")),
                cover_url: meta.cover_url,
                ..Default::default()
            });
        }
        None
    }

    fn extract_from_json(&self, dir: &Path) -> Option<ScannedMetadata> {
        match crate::core::metadata_writer::read_metadata_json(dir) {
            Ok(Some(meta)) => {
                let mut m = ScannedMetadata::default();
                m.title = meta.title;
                if !meta.authors.is_empty() { m.author = Some(meta.authors[0].clone()); }
                if !meta.narrators.is_empty() { m.narrator = Some(meta.narrators[0].clone()); }
                m.description = meta.description;
                if !meta.genres.is_empty() { m.genre = Some(meta.genres.join(",")); }
                if !meta.tags.is_empty() { 
                    m.json_tags = meta.tags.clone();
                    m.tags = Some(meta.tags.join(","));
                }
                m.json_series = meta.series;
                m.subtitle = meta.subtitle;
                m.published_year = meta.published_year;
                m.published_date = meta.published_date;
                m.publisher = meta.publisher;
                m.isbn = meta.isbn;
                m.asin = meta.asin;
                m.language = meta.language;
                m.explicit = meta.explicit;
                m.abridged = meta.abridged;
                if !meta.chapters.is_empty() { m.json_chapters = Some(meta.chapters); }
                Some(m)
            },
            Ok(None) => None,
            Err(e) => {
                warn!("Failed to read metadata.json in {:?}: {}", dir, e);
                None
            }
        }
    }

    async fn extract_from_audio(&self, _dir: &Path, files: &[PathBuf], extract_cover: bool) -> Option<ScannedMetadata> {
        if files.is_empty() { return None; }
        let file_path = &files[0];
        let mut m = ScannedMetadata::default();
        let mut found = false;

        let ext = file_path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let is_standard = STANDARD_EXTENSIONS.contains(&ext.as_str());

        if is_standard {
             if let Ok(meta) = self.audio_streamer.read_metadata(file_path) {
                 if let Some(t) = meta.album {
                     if !t.trim().is_empty() {
                         m.title = Some(t);
                         found = true;
                     }
                 }
                 if let Some(aa) = meta.album_artist {
                      if !aa.trim().is_empty() { m.author = Some(aa); }
                 }
                 if let Some(a) = meta.artist {
                     if !a.trim().is_empty() {
                         if m.author.is_none() { m.author = Some(a.clone()); }
                         else if m.author.as_ref() != Some(&a) { m.narrator = Some(a); }
                     }
                 }
                 if let Some(c) = meta.composer {
                     if !c.trim().is_empty() && m.narrator.is_none() { m.narrator = Some(c); }
                 }
                 if let Some(g) = meta.genre {
                     if !g.trim().is_empty() { m.genre = Some(g); }
                 }
                 // In v1.2.0, standard files (like .m4a, .mp3) ONLY extracted cover in the fallback step
                 // OR they extracted it here if we specifically added it. 
                 // Wait, symphonia's audio_streamer DOES NOT extract cover. 
                 // The old code ONLY extracted cover in the fallback step (which was restricted to mp3).
                 // So in v1.2.0 m4a cover extraction was completely broken or relied on `extract_and_save_cover`.
                 // Let's use `extract_and_save_cover` here for ALL standard files, not just mp3.
                 if extract_cover {
                     if let Some(path) = self.extract_and_save_cover(file_path, _dir) {
                         m.cover_url = Some(path);
                         found = true;
                     }
                 }
             }
        }

        // Try plugins if title empty or not standard, OR if we need cover but didn't find one
        // Also call plugin if extract_cover is true to ensure we attempt cover extraction even if title was found by symphonia
        if m.title.is_none() || !is_standard || (extract_cover && m.cover_url.is_none()) {
             let plugins = self.plugin_manager.find_plugins_by_type(PluginType::Format).await;
             for plugin in plugins {
                 let supports_ext = plugin.supported_extensions.as_ref()
                     .map(|exts| exts.iter().any(|e| e.eq_ignore_ascii_case(&ext)))
                     .unwrap_or(false);
                 if !supports_ext { continue; }

                 let params = serde_json::json!({ "file_path": file_path.to_string_lossy(), "extract_cover": extract_cover });
                 if let Ok(result) = self.plugin_manager.call_format(&plugin.id, FormatMethod::ExtractMetadata, params).await {
                     if let Some(t) = result.get("album").and_then(|v| v.as_str()) {

                         if !t.trim().is_empty() {
                             m.title = Some(t.to_string());
                             found = true;
                         }
                     }
                     if let Some(aa) = result.get("album_artist").and_then(|v| v.as_str()) {
                         if !aa.trim().is_empty() { m.author = Some(aa.to_string()); }
                     }
                     if let Some(a) = result.get("artist").and_then(|v| v.as_str()) {
                         if !a.trim().is_empty() {
                             if m.author.is_none() { m.author = Some(a.to_string()); }
                             else if m.author.as_ref().map(|s| s.as_str()) != Some(a) { m.narrator = Some(a.to_string()); }
                         }
                     }
                     if let Some(n) = result.get("narrator").and_then(|v| v.as_str()) {
                         if !n.trim().is_empty() { m.narrator = Some(n.to_string()); }
                     }
                     if extract_cover && m.cover_url.is_none() {
                         if let Some(c) = result.get("cover_url").and_then(|v| v.as_str()) {
                             if !c.trim().is_empty() { 
                                 m.cover_url = Some(c.to_string()); 
                                 found = true;
                             }
                         }
                     }
                     if let Some(d) = result.get("description").and_then(|v| v.as_str()) {
                         if !d.trim().is_empty() { m.description = Some(d.to_string()); }
                     }
                     if let Some(g) = result.get("genre").and_then(|v| v.as_str()) {
                         if !g.trim().is_empty() { m.genre = Some(g.to_string()); }
                     }
                     
                     // If we found basic metadata (found=true) AND (either we don't need a cover, or we found a cover)
                     if found && (!extract_cover || m.cover_url.is_some()) {
                         break; 
                     }
                 }
             }
        }

        info!("extract_from_audio: returning found={}, meta={:?}", found, m);
        if found { Some(m) } else { None }
    }

    async fn extract_from_scraper(&self, title: &str, _author: &Option<String>, scraper_config: &crate::db::models::ScraperConfig) -> Option<ScannedMetadata> {
        if let Some(scraper) = &self.scraper_service {
             // Basic scrape check
             if let Ok(detail) = scraper.scrape_book_metadata(title, scraper_config).await {
                 let mut m = ScannedMetadata::default();
                 if !detail.intro.is_empty() { m.description = Some(detail.intro); }
                 if !detail.tags.is_empty() { m.tags = Some(detail.tags.join(",")); }
                 if let Some(g) = detail.genre { if !g.trim().is_empty() { m.genre = Some(g); } }
                 m.cover_url = detail.cover_url;
                 m.narrator = detail.narrator;
                 if !detail.author.is_empty() { m.author = Some(detail.author); }
                 m.subtitle = detail.subtitle;
                 m.published_year = detail.published_year;
                 m.published_date = detail.published_date;
                 m.publisher = detail.publisher;
                 m.isbn = detail.isbn;
                 m.asin = detail.asin;
                 m.language = detail.language;
                 if detail.explicit { m.explicit = true; }
                 if detail.abridged { m.abridged = true; }
                 return Some(m);
             }
        }
        None
    }

    fn extract_and_save_cover(&self, audio_path: &Path, book_dir: &Path) -> Option<String> {
        // Check if cover file already exists in the directory
        // Common cover file patterns: cover.jpg, cover.png, cover.webp, cover.gif
        let cover_extensions = ["jpg", "jpeg", "png", "webp", "gif"];
        for ext in &cover_extensions {
            let cover_path = book_dir.join(format!("cover.{}", ext));
            if cover_path.exists() {
                info!("Cover file already exists at {:?}, skipping extraction", cover_path);
                return Some(cover_path.to_string_lossy().replace('\\', "/"));
            }
        }
        
        // No existing cover found, proceed with extraction
        // We use id3 library here, which mainly supports MP3 (ID3v2 tags).
        // For M4A, id3 library might fail. We should check if we can extract M4A covers too.
        // The id3 crate only supports ID3v1 and ID3v2 tags, not MP4/M4A metadata.
        // Wait! In v1.2.0, the `native-audio-support` plugin was used for M4A.
        // Let's first try id3 tag.
        if let Ok(tag) = id3::Tag::read_from_path(audio_path) {
            // Prefer CoverFront, otherwise take the first picture
            let picture = tag.pictures()
                .find(|p| p.picture_type == id3::frame::PictureType::CoverFront)
                .or_else(|| tag.pictures().next());

            if let Some(picture) = picture {
                // Determine extension from mime type
                let ext = match picture.mime_type.as_str() {
                    "image/jpeg" | "image/jpg" => "jpg",
                    "image/png" => "png",
                    "image/webp" => "webp",
                    "image/gif" => "gif",
                    _ => "jpg", // Default to jpg
                };
                
                let cover_filename = format!("cover.{}", ext);
                let cover_path = book_dir.join(&cover_filename);
                
                // Save to file
                if let Err(e) = std::fs::write(&cover_path, &picture.data) {
                    warn!("Failed to save extracted cover to {:?}: {}", cover_path, e);
                    return None;
                }
                
                info!("Extracted cover from ID3 tag to {:?}", cover_path);
                return Some(cover_path.to_string_lossy().replace('\\', "/"));
            }
        }
        None
    }

    pub(crate) async fn process_chapters(
        &self, 
        book_id: &str, 
        files: &[PathBuf], 
        last_scanned: Option<chrono::DateTime<chrono::Utc>>,
        task_id: Option<&str>,
        use_filename_as_title: bool,
        json_chapters: Option<Vec<crate::core::metadata_writer::AudiobookshelfChapter>>,
    ) -> Result<bool> {
        let mut has_changes = false;
        let total_files = files.len();
        
        // Use JSON chapters if available and count matches
        let use_json_chapters = if let Some(ref chapters) = json_chapters {
            if chapters.len() == total_files {
                info!("Using metadata.json chapters for book_id: {}", book_id);
                true
            } else {
                if !chapters.is_empty() {
                    warn!("metadata.json chapter count ({}) does not match file count ({}) for book {}. Ignoring JSON chapters.", chapters.len(), total_files, book_id);
                }
                false
            }
        } else {
            false
        };
        
        // Fetch book to check for regex rule
        let book = self.book_repo.find_by_id(book_id).await?
            .ok_or_else(|| TingError::NotFound("Book not found".to_string()))?;
            
        let chapter_regex = if let Some(pattern) = &book.chapter_regex {
            regex::Regex::new(pattern).ok()
        } else {
            None
        };

        // Pre-fetch existing chapters to support efficient incremental scanning
        // Map Path -> Chapter
        let existing_chapters = self.chapter_repo.find_by_book(book_id).await?;
        let mut chapter_map: HashMap<PathBuf, Chapter> = HashMap::new();
        for ch in existing_chapters {
            let p = PathBuf::from(&ch.path);
            chapter_map.insert(p, ch);
        }

        let mut main_counter = 0;
        let mut extra_counter = 0;
        
        // Track processed chapter IDs to find deleted ones
        let mut processed_chapter_ids = HashSet::new();

        for (index, file_path) in files.iter().enumerate() {
            if index % 5 == 0 {
                // Check cancellation and log progress
                self.check_cancellation(task_id).await?;
                self.update_progress(task_id, format!("处理章节 {}/{}", index + 1, total_files)).await;
            }

            // Incremental Scan Logic
            // Check if file exists in DB
            let mut existing_chapter = chapter_map.get(file_path).cloned();
            
            // Check if file has changed
            let is_modified = if let Some(last_scan) = last_scanned {
                if let Ok(metadata) = std::fs::metadata(file_path) {
                    if let Ok(mtime) = metadata.modified() {
                        let mtime_utc: chrono::DateTime<chrono::Utc> = mtime.into();
                        mtime_utc > last_scan
                    } else {
                        true // Can't read mtime, force check
                    }
                } else {
                    true // Can't read metadata, force check
                }
            } else {
                true // No last scan, force check
            };

            // Common Logic: Calculate Regex/Filename properties
            let filename_str = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string();
            let mut regex_idx = None;
            let mut regex_title = None;
            
            if let Some(re) = &chapter_regex {
                if let Some(caps) = re.captures(&filename_str) {
                        if let Some(m) = caps.get(1) {
                            if let Ok(idx) = m.as_str().parse::<i32>() {
                                regex_idx = Some(idx);
                            }
                        }
                        if let Some(m) = caps.get(2) {
                            regex_title = Some(m.as_str().to_string());
                        }
                }
            }

            // Optimization: If chapter exists and file is not modified, skip processing!
            if let Some(ref ch) = existing_chapter {
                if !is_modified {
                    // Update index if needed (e.g. reordering files), but skip hashing/metadata
                    // Also respect manual_corrected if we were to update anything else
                    
                    let is_extra_ch = ch.is_extra == 1;
                    let idx_from_counter = if is_extra_ch {
                         extra_counter += 1;
                         extra_counter
                    } else {
                         main_counter += 1;
                         main_counter
                    };
                    
                    // Final Index (Regex overrides counter)
            let target_idx = regex_idx.unwrap_or(idx_from_counter);

                    // Check if we need to update Title or Index
                    // Cases to update even if not modified:
                    // 1. Regex applied/changed and provides new title/index.
                    // 2. use_filename_as_title is TRUE and current title != filename.
                    // 3. Index changed due to reordering.
                    
                    let mut should_update = false;
                    let mut new_title = ch.title.clone();
                    let mut new_idx = ch.chapter_index;

                    // Check Index
                    if new_idx != Some(target_idx) {
                        new_idx = Some(target_idx);
                        should_update = true;
                    }

                    // Check Title
                    // If JSON chapters used, we don't touch title here (it's from JSON)
                    // If Regex Title exists, use it.
                    // If use_filename_as_title, use filename.
                    if !use_json_chapters && ch.manual_corrected == 0 {
                         let target_title = if let Some(rt) = regex_title.clone() {
                             // Apply text cleaner to regex result as requested
                             let (cleaned, _) = self.text_cleaner.clean_chapter_title(&rt, book.title.as_deref());
                             cleaned
                         } else if use_filename_as_title {
                             let (cleaned, _) = self.text_cleaner.clean_chapter_title(&filename_str, book.title.as_deref());
                             cleaned
                         } else {
                             // If not forced and no regex, keep existing title (audio or whatever it was)
                             // Unless we want to re-run text cleaner?
                             // Let's assume existing title is fine if no config change.
                             ch.title.clone().unwrap_or_default()
                         };
                         
                         if ch.title.as_deref() != Some(&target_title) {
                             // Only update if we are forcing filename OR regex provided a title
                             if use_filename_as_title || regex_title.is_some() {
                                 new_title = Some(target_title);
                                 should_update = true;
                             }
                         }
                    }

                    if should_update && ch.manual_corrected == 0 {
                         let mut updated_ch = ch.clone();
                         updated_ch.chapter_index = new_idx;
                         updated_ch.title = new_title;
                         self.chapter_repo.update(&updated_ch).await?;
                         has_changes = true;
                    }
                    processed_chapter_ids.insert(ch.id.clone());
                    continue;
                }
            }

            // If we are here, either it's a new file OR it's modified.
            
            // Calculate content-based hash
            let file_hash = self.calculate_file_hash(file_path)?;

            // Check if chapter exists by Hash (Global Deduplication)
            // But we must be careful: if we already found it by Path, we know it's that chapter.
            // If we found by Path, but Hash changed, it's an update.
            // If we didn't find by Path, we check Hash to see if it's a move/rename.
            
            if existing_chapter.is_none() {
                if let Ok(Some(ch)) = self.chapter_repo.find_by_hash(&file_hash).await {
                    // Found by hash (Rename/Move case)
                    // But we are processing a specific book_id here.
                    // If the found chapter belongs to another book, we might be stealing it?
                    // Or it's a duplicate file (e.g. same intro file in multiple books).
                    // If it's the same book, we treat it as the "existing chapter".
                    if ch.book_id == book_id {
                        existing_chapter = Some(ch);
                    }
                    // If different book, we create a new chapter record (duplicate content allowed across books)
                }
            }

            // Extract metadata
            // If using JSON chapters, calculate duration from JSON (end - start)
            let duration = if use_json_chapters {
                if let Some(ref chapters) = json_chapters {
                    if index < chapters.len() {
                        let chapter = &chapters[index];
                        ((chapter.end - chapter.start).round() as i32).max(0)
                    } else {
                        // Fallback to file extraction
                        let (_, _, _, _, _, d) = self.extract_chapter_metadata(file_path).await;
                        d
                    }
                } else {
                    // Fallback to file extraction
                    let (_, _, _, _, _, d) = self.extract_chapter_metadata(file_path).await;
                    d
                }
            } else {
                // Extract from file
                let (_, _, _, _, _, d) = self.extract_chapter_metadata(file_path).await;
                d
            };
            
            // Extract title (only if not using JSON chapters)
            let extracted_title = if !use_json_chapters {
                let (_, t, _, _, _, _) = self.extract_chapter_metadata(file_path).await;
                t
            } else {
                String::new()
            };
            
            // Determine initial title based on preference
            let mut title = if use_json_chapters {
                // Priority 1: metadata.json (if count matches)
                if let Some(ref chapters) = json_chapters {
                    if index < chapters.len() {
                        chapters[index].title.clone()
                    } else {
                        // Should not happen if use_json_chapters is true
                        filename_str.clone()
                    }
                } else {
                    filename_str.clone()
                }
            } else if use_filename_as_title {
                filename_str.clone()
            } else if !extracted_title.is_empty() {
                // Default: Audio Title
                extracted_title
            } else {
                filename_str.clone()
            };

            // Regex Title Override (if not using JSON)
            if !use_json_chapters {
                if let Some(rt) = regex_title {
                    title = rt;
                }
            }
            
            // Apply text cleaner to title
            let raw_title = title;
            
            let (final_title, is_extra) = if use_json_chapters {
                (raw_title, false)
            } else {
                self.text_cleaner.clean_chapter_title(&raw_title, book.title.as_deref())
            };

            // Calculate Index using counters
            let counter_idx = if is_extra {
                 extra_counter += 1;
                 extra_counter
            } else {
                 main_counter += 1;
                 main_counter
            };
            
            // Final Index
            let chapter_idx = regex_idx.unwrap_or(counter_idx);

            if let Some(mut ch) = existing_chapter {
                // Update Existing
                // Check Lock
                if ch.manual_corrected == 0 {
                    ch.title = Some(final_title);
                    ch.chapter_index = Some(chapter_idx);
                    ch.is_extra = if is_extra { 1 } else { 0 };
                }
                // Always update duration/path/hash if file changed
                ch.path = file_path.to_string_lossy().to_string();
                ch.duration = Some(duration);
                ch.hash = Some(file_hash);
                ch.book_id = book_id.to_string();
                
                self.chapter_repo.update(&ch).await?;
                has_changes = true;
                processed_chapter_ids.insert(ch.id.clone());
            } else {
                // Create New
                let chapter_id = Uuid::new_v4().to_string();
                let chapter = Chapter {
                    id: chapter_id.clone(),
                    book_id: book_id.to_string(),
                    title: Some(final_title),
                    path: file_path.to_string_lossy().to_string(),
                    duration: Some(duration),
                    chapter_index: Some(chapter_idx),
                    is_extra: if is_extra { 1 } else { 0 },
                    hash: Some(file_hash),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    manual_corrected: 0,
                };
                
                 match self.chapter_repo.create(&chapter).await {
                     Ok(_) => { 
                         has_changes = true; 
                         processed_chapter_ids.insert(chapter_id);
                     },
                     Err(e) => warn!("Failed to create chapter: {}", e),
                 }
            }
        }
        
        // Handle deleted chapters
        for (path, ch) in chapter_map {
            if !processed_chapter_ids.contains(&ch.id) {
                // The chapter file is missing, remove from DB
                if !path.exists() {
                    info!("Removing missing chapter from DB: {:?}", path);
                    if let Err(e) = self.chapter_repo.delete(&ch.id).await {
                        warn!("Failed to delete missing chapter {}: {}", ch.id, e);
                    } else {
                        has_changes = true;
                    }
                }
            }
        }
        
        Ok(has_changes)
    }

    fn find_cover_image(&self, dir: &Path) -> Option<String> {
        let cover_names = ["cover.jpg", "cover.png", "cover.jpeg", "folder.jpg", "folder.png"];
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

    fn calculate_file_hash(&self, path: &Path) -> Result<String> {
        let mut file = std::fs::File::open(path).map_err(|e| TingError::IoError(e))?;
        let metadata = file.metadata().map_err(|e| TingError::IoError(e))?;
        let len = metadata.len();
        
        let mut buffer = vec![0; 16384]; // 16KB
        let n = file.read(&mut buffer).map_err(|e| TingError::IoError(e))?;
        
        let mut hasher = Sha256::new();
        hasher.update(&buffer[..n]);
        hasher.update(len.to_le_bytes());
        // Also include filename to distinguish different chapters with same content/size (unlikely but possible)
        if let Some(name) = path.file_name() {
             hasher.update(name.to_string_lossy().as_bytes());
        }

        Ok(format!("{:x}", hasher.finalize()))
    }
}
