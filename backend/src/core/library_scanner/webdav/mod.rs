mod listing;
mod metadata;
mod processing;

use super::{LibraryScanner, ScanMode, ScanResult, ScanStatus};
use crate::core::error::{Result, TingError};
use crate::core::library_scanner::shared::{
    infer_series_directories, parse_chapter_range_dir_name, select_mergeable_range_groups,
    ChapterRangeDir, CoalescedRangeDirs, SeriesDirectoryCandidate,
};
use crate::db::repository::Repository;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, warn};

type WebDavFileEntry = (String, Option<chrono::DateTime<chrono::Utc>>);

impl LibraryScanner {
    /// Scan a WebDAV library
    pub(crate) async fn scan_webdav_library(
        &self,
        library: &crate::db::models::Library,
        task_id: Option<&str>,
        scraper_config: &crate::db::models::ScraperConfig,
        mode: ScanMode,
    ) -> Result<ScanResult> {
        if self.storage_service.is_none() {
            return Err(TingError::ConfigError(
                "Storage service not configured for WebDAV scan".to_string(),
            ));
        }

        let mut scan_result = ScanResult::default();
        scan_result.start_time = Some(std::time::Instant::now());
        self.update_progress_key(task_id, "scan.webdav.scanning", serde_json::json!({}))
            .await;

        // 1. List files recursively
        let files = self.list_webdav_files(library, task_id).await?;

        let supported_extensions = self.get_supported_extensions().await;

        // Group by directory URL (parent URL)
        // Key: Parent URL (String), Value: List of (File URL, Last Modified)
        let mut dir_groups: HashMap<String, Vec<WebDavFileEntry>> = HashMap::new();

        // Metadata/sidecar file extensions that should be grouped alongside audio files
        // so that cover images, metadata.json, book.nfo etc. are available during processing.
        const METADATA_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "json", "nfo"];

        for (file_url, last_mod) in files {
            // Check extension
            if let Some(ext_pos) = file_url.rfind('.') {
                let ext = file_url[ext_pos + 1..].to_lowercase();
                if supported_extensions.contains(&ext)
                    || METADATA_EXTENSIONS.contains(&ext.as_str())
                {
                    // Get parent URL
                    if let Some(last_slash) = file_url.rfind('/') {
                        let parent = file_url[0..last_slash].to_string();
                        dir_groups
                            .entry(parent)
                            .or_default()
                            .push((file_url, last_mod));
                    }
                }
            }
        }

        self.update_progress_key(
            task_id,
            "scan.audio_dirs.found",
            serde_json::json!({ "count": dir_groups.len() }),
        )
        .await;

        let (dir_groups, coalesced_range_dirs) =
            self.coalesce_webdav_range_directory_groups(dir_groups);
        let inferred_series = self.infer_webdav_series_directories(dir_groups.keys());
        let total_groups = dir_groups.len();
        let mut processed_count = 0;

        // Pre-fetch all books for lookup and deletion handling
        let prefetched = self.prefetch_books(&library.id).await;

        let mut book_path_map: HashMap<String, (String, i32, Option<String>)> = HashMap::new();
        let mut book_hash_map: HashMap<String, (String, i32, Option<String>)> = HashMap::new();
        for (id, path, hash, manual_corrected, match_pattern) in &prefetched.all_books {
            book_path_map.insert(
                path.clone(),
                (id.clone(), *manual_corrected, match_pattern.clone()),
            );
            book_hash_map.insert(
                hash.clone(),
                (id.clone(), *manual_corrected, match_pattern.clone()),
            );
        }

        let mut found_book_ids: HashSet<String> = HashSet::new();
        let mut absorbed_range_book_ids: HashMap<String, String> = HashMap::new();
        let last_scanned = if mode.is_full() {
            None
        } else if let Some(ref date_str) = library.last_scanned_at {
            chrono::DateTime::parse_from_rfc3339(date_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok()
        } else {
            None
        };

        for (dir_url, mut file_entries) in dir_groups {
            // Check cancellation
            self.check_cancellation(task_id).await?;

            processed_count += 1;
            // Extract directory name from URL for logging
            let dir_name = self.webdav_url_name(&dir_url);

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

            // Sort file entries naturally by URL
            file_entries.sort_by(|a, b| natord::compare(&a.0, &b.0));

            // Extract just URLs for processing
            let mut file_urls: Vec<String> = Vec::new();
            let mut metadata_files: Vec<String> = Vec::new();

            for (url, _) in file_entries.iter() {
                let ext = url.split('.').last().unwrap_or_default().to_lowercase();
                if ["json", "nfo", "jpg", "png", "jpeg", "webp"].contains(&ext.as_str()) {
                    metadata_files.push(url.clone());
                } else {
                    file_urls.push(url.clone());
                }
            }

            // Skip directories with no audio files (only metadata/sidecar files)
            if file_urls.is_empty() {
                continue;
            }

            // Calculate directory hash for lookup
            let mut hasher = Sha256::new();
            hasher.update(dir_url.as_bytes());
            let dir_hash = format!("{:x}", hasher.finalize());

            // Optimization: Find existing book to avoid DB lookup
            let mut existing_info = book_path_map.get(&dir_url).cloned();
            if existing_info.is_none() {
                existing_info = book_hash_map.get(&dir_hash).cloned();
            }
            if existing_info.is_none() {
                if let Some(child_dirs) = coalesced_range_dirs.get(&dir_url) {
                    for child_dir in &child_dirs.child_dirs {
                        if let Some(info) = book_path_map.get(child_dir).cloned() {
                            existing_info = Some(info);
                            break;
                        }
                    }
                }
            }

            // Incremental Check: Skip if book exists and no files modified since last scan
            if let (Some((id, _, _)), Some(last_scan_time)) = (&existing_info, last_scanned) {
                // Check if file count changed (new files added or removed)
                let current_file_count = file_urls.len();
                let existing_chapters =
                    self.chapter_repo.find_by_book(id).await.unwrap_or_default();
                let existing_chapter_count = existing_chapters.len();

                // Determine latest modification time in this directory
                let max_mtime = file_entries.iter().filter_map(|(_, mtime)| *mtime).max();

                let mtime_count = file_entries
                    .iter()
                    .filter(|(_, mtime)| mtime.is_some())
                    .count();

                info!(
                    book_id = %id,
                    url = %dir_url,
                    max_mtime = ?max_mtime,
                    last_scan_time = %last_scan_time,
                    total_files = file_entries.len(),
                    files_with_mtime = mtime_count,
                    current_file_count = current_file_count,
                    existing_chapter_count = existing_chapter_count,
                    "Checking if WebDAV book needs update"
                );

                // Skip only if:
                // 1. File count hasn't changed AND
                // 2. No files have been modified since last scan
                if current_file_count == existing_chapter_count {
                    if let Some(latest) = max_mtime {
                        if latest <= last_scan_time {
                            let should_reprocess_chapter_titles = self
                                .book_repo
                                .find_by_id(id)
                                .await?
                                .as_ref()
                                .map(|book| {
                                    self.webdav_chapter_title_rules_need_reprocess(
                                        book,
                                        &existing_chapters,
                                        &file_urls,
                                        scraper_config,
                                    )
                                })
                                .unwrap_or(false);

                            if should_reprocess_chapter_titles {
                                info!(
                                    book_id = %id,
                                    url = %dir_url,
                                    "WebDAV book is up to date but chapter title rules changed, will reprocess chapters"
                                );
                            } else {
                                // Book exists and is up to date
                                scan_result.total_books += 1;
                                scan_result.books_skipped += 1;
                                found_book_ids.insert(id.clone());
                                if let Some(series_info) = inferred_series.get(&dir_url) {
                                    if let Err(e) = self
                                        .link_book_to_inferred_series(&library.id, id, series_info)
                                        .await
                                    {
                                        warn!(
                                            url = %dir_url,
                                            book_id = %id,
                                            error = %e,
                                            "Failed to link skipped WebDAV book to inferred series"
                                        );
                                    }
                                }
                                info!(book_id = %id, url = %dir_url, "Skipping up-to-date WebDAV book");
                                continue;
                            }
                        } else {
                            info!(book_id = %id, url = %dir_url, "WebDAV book has newer files, will update");
                        }
                    } else {
                        // No modification times available, force update
                        info!(book_id = %id, url = %dir_url, "No modification times available, will process book");
                    }
                } else {
                    info!(
                        book_id = %id,
                        url = %dir_url,
                        "File count changed ({} -> {}), will process book",
                        existing_chapter_count,
                        current_file_count
                    );
                }
            }

            match self
                .process_webdav_book(
                    library,
                    &dir_url,
                    &file_urls,
                    &metadata_files,
                    task_id,
                    scraper_config,
                    existing_info,
                    coalesced_range_dirs
                        .get(&dir_url)
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
                    if let Some(series_info) = inferred_series.get(&dir_url) {
                        if let Err(e) = self
                            .link_book_to_inferred_series(&library.id, &book_id, series_info)
                            .await
                        {
                            warn!(
                                url = %dir_url,
                                book_id = %book_id,
                                error = %e,
                                "Failed to link WebDAV book to inferred series"
                            );
                        }
                    }
                    if let Some(child_dirs) = coalesced_range_dirs.get(&dir_url) {
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
                    debug!(book_id = %book_id, url = %dir_url, status = ?status, "Processed WebDAV book directory");
                }
                Err(e) => {
                    scan_result.failed_count += 1;
                    warn!(url = %dir_url, error = %e, "Failed to process WebDAV book directory");
                    scan_result
                        .errors
                        .push(format!("Failed to process {}: {}", dir_url, e));
                }
            }

            // Periodic garbage collection
            self.plugin_manager.garbage_collect_all().await;
        }

        if let Some(merge_service) = &self.merge_service {
            for (source_id, target_id) in absorbed_range_book_ids {
                if found_book_ids.contains(&source_id) {
                    continue;
                }

                if let Err(e) = merge_service
                    .absorb_scanned_book(&target_id, &source_id)
                    .await
                {
                    warn!(
                        "Failed to absorb WebDAV range-segment book {} into {}: {}",
                        source_id, target_id, e
                    );
                } else {
                    scan_result.books_deleted += 1;
                    found_book_ids.insert(source_id);
                }
            }
        }

        // 3. Handle deletions via shared helper (no path-exists check for WebDAV)
        self.handle_book_deletions(&mut scan_result, &prefetched, &found_book_ids, false)
            .await;

        // Final garbage collection after scan
        self.plugin_manager.garbage_collect_all().await;

        Ok(scan_result)
    }

    fn webdav_chapter_title_rules_need_reprocess(
        &self,
        book: &crate::db::models::Book,
        existing_chapters: &[crate::db::models::Chapter],
        file_urls: &[String],
        scraper_config: &crate::db::models::ScraperConfig,
    ) -> bool {
        let chapter_regex = book
            .chapter_regex
            .as_ref()
            .filter(|pattern| !pattern.trim().is_empty())
            .and_then(|pattern| regex::Regex::new(pattern).ok());

        if chapter_regex.is_none() && !scraper_config.use_filename_as_title {
            return existing_chapters.iter().any(|chapter| {
                if chapter.manual_corrected != 0 {
                    return false;
                }

                let Some(title) = chapter.title.as_deref() else {
                    return false;
                };

                let (_, is_extra) = self
                    .text_cleaner
                    .clean_chapter_title(title, book.title.as_deref());
                chapter.is_extra != if is_extra { 1 } else { 0 }
            });
        }

        let mut chapter_by_hash = HashMap::new();
        let mut chapter_by_path = HashMap::new();
        for chapter in existing_chapters {
            if let Some(hash) = &chapter.hash {
                chapter_by_hash.insert(hash.as_str(), chapter);
            }
            chapter_by_path.insert(chapter.path.as_str(), chapter);
        }

        let mut main_counter = 0;
        let mut extra_counter = 0;

        for file_url in file_urls {
            let mut hasher = Sha256::new();
            hasher.update(file_url.as_bytes());
            let chapter_hash = format!("{:x}", hasher.finalize());

            let Some(chapter) = chapter_by_hash
                .get(chapter_hash.as_str())
                .or_else(|| chapter_by_path.get(file_url.as_str()))
            else {
                return true;
            };

            let decoded_file_url = self.decode_url_path(file_url);
            let filename = decoded_file_url
                .split('/')
                .last()
                .unwrap_or("chapter")
                .to_string();

            let mut regex_idx = None;
            let mut regex_title = None;
            if let Some(re) = &chapter_regex {
                if let Some(caps) = re.captures(&filename) {
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

            let title_override = if let Some(rt) = regex_title {
                let (cleaned, is_extra) = self
                    .text_cleaner
                    .clean_chapter_title(&rt, book.title.as_deref());
                Some((cleaned, is_extra))
            } else if scraper_config.use_filename_as_title {
                let (cleaned, is_extra) = self
                    .text_cleaner
                    .clean_chapter_title(&filename, book.title.as_deref());
                Some((cleaned, is_extra))
            } else {
                None
            };

            let counter_is_extra = title_override
                .as_ref()
                .map(|(_, is_extra)| *is_extra)
                .unwrap_or(chapter.is_extra == 1);
            let counter_idx = if counter_is_extra {
                extra_counter += 1;
                extra_counter
            } else {
                main_counter += 1;
                main_counter
            };
            let target_idx = regex_idx.unwrap_or(counter_idx);

            if chapter.manual_corrected != 0 {
                continue;
            }

            if chapter.chapter_index != Some(target_idx) {
                return true;
            }

            if let Some((target_title, target_is_extra)) = title_override {
                if chapter.title.as_deref() != Some(target_title.as_str()) {
                    return true;
                }

                let target_is_extra = if target_is_extra { 1 } else { 0 };
                if chapter.is_extra != target_is_extra {
                    return true;
                }
            }
        }

        false
    }

    fn coalesce_webdav_range_directory_groups(
        &self,
        mut dir_groups: HashMap<String, Vec<WebDavFileEntry>>,
    ) -> (
        HashMap<String, Vec<WebDavFileEntry>>,
        HashMap<String, CoalescedRangeDirs<String>>,
    ) {
        let mut candidates: HashMap<String, Vec<(String, ChapterRangeDir)>> = HashMap::new();

        for dir_url in dir_groups.keys() {
            let Some(parent_url) = webdav_parent_url(dir_url) else {
                continue;
            };
            let dir_name = self.webdav_url_name(dir_url);
            let Some(range_dir) = parse_chapter_range_dir_name(&dir_name) else {
                continue;
            };

            candidates
                .entry(parent_url)
                .or_default()
                .push((dir_url.clone(), range_dir));
        }

        let mut coalesced_range_dirs = HashMap::new();

        for (parent_url, entries) in candidates {
            let parent_name = self.webdav_url_name(&parent_url);
            let ranges: Vec<ChapterRangeDir> =
                entries.iter().map(|(_, range)| range.clone()).collect();

            for group in select_mergeable_range_groups(&parent_name, &ranges) {
                let mut selected: Vec<(String, ChapterRangeDir)> = group
                    .indices
                    .into_iter()
                    .map(|index| entries[index].clone())
                    .collect();
                selected.sort_by_key(|(_, range)| (range.start, range.end));

                let Some(first_child_dir) =
                    selected.first().map(|(child_dir, _)| child_dir.clone())
                else {
                    continue;
                };

                let target_dir = if group.merge_into_parent {
                    parent_url.clone()
                } else {
                    first_child_dir
                };

                let child_dirs: Vec<String> = selected
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

    fn webdav_url_name(&self, url: &str) -> String {
        let raw_name = url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("");
        self.decode_url_path(raw_name)
    }

    fn infer_webdav_series_directories<'a>(
        &self,
        dirs: impl Iterator<Item = &'a String>,
    ) -> HashMap<String, crate::core::library_scanner::shared::InferredSeriesInfo> {
        let candidates: Vec<SeriesDirectoryCandidate<String>> = dirs
            .filter_map(|dir_url| {
                let parent_url = webdav_parent_url(dir_url)?;
                let dir_name = self.webdav_url_name(dir_url);
                let parent_name = self.webdav_url_name(&parent_url);
                Some(SeriesDirectoryCandidate {
                    key: dir_url.clone(),
                    parent_key: parent_url,
                    parent_name,
                    name: dir_name,
                })
            })
            .collect();

        infer_series_directories(&candidates)
    }
}

fn webdav_parent_url(url: &str) -> Option<String> {
    let trimmed = url.trim_end_matches('/');
    let slash_index = trimmed.rfind('/')?;
    if slash_index == 0 {
        return None;
    }
    Some(trimmed[..slash_index].to_string())
}
