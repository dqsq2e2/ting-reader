use super::super::LibraryScanner;
use super::super::shared::{
    apply_chapter_title_template, chapter_title_template_preserves_raw,
    clean_or_preserve_chapter_title,
};
use crate::core::app::error::LookupErrorKind;
use crate::core::app::error::{Result, TingError};
use crate::db::models::Chapter;
use crate::db::repository::Repository;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;

pub(super) struct ChapterProcessingOptions<'a> {
    pub(super) last_scanned: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) task_id: Option<&'a str>,
    pub(super) scraper_config: &'a crate::db::models::ScraperConfig,
    pub(super) changed_file_paths: Option<&'a HashSet<PathBuf>>,
    pub(super) json_chapters: Option<Vec<crate::core::books::metadata::AudiobookshelfChapter>>,
    pub(super) chapter_title_template: Option<&'a str>,
    pub(super) chapter_title_overrides: Option<&'a [String]>,
    /// Whether a single-file MP4 may replace its chapter list with the
    /// chapters embedded in the file. Disabled when the caller merges an extra
    /// directory into an existing book.
    pub(super) probe_embedded_chapters: bool,
}

impl LibraryScanner {
    pub(super) async fn process_chapters(
        &self,
        book_id: &str,
        files: &[PathBuf],
        options: ChapterProcessingOptions<'_>,
    ) -> Result<bool> {
        let ChapterProcessingOptions {
            last_scanned,
            task_id,
            scraper_config,
            changed_file_paths,
            json_chapters,
            chapter_title_template,
            chapter_title_overrides,
            probe_embedded_chapters,
        } = options;
        let extract_extra_chapters = scraper_config.extract_extra_chapters;
        let cloud_mode = scraper_config.cloud_mode;
        let mut has_changes = false;
        let total_files = files.len();
        let mut json_chapters = json_chapters;

        // Use JSON chapters if available and count matches
        let use_json_chapters = if let Some(ref chapters) = json_chapters {
            if chapters.len() == total_files {
                info!("Using metadata.json chapters for book_id: {}", book_id);
                true
            } else {
                if !chapters.is_empty() {
                    warn!(
                        "metadata.json chapter count ({}) does not match file count ({}) for book {}. Ignoring JSON chapters.",
                        chapters.len(),
                        total_files,
                        book_id
                    );
                }
                false
            }
        } else {
            false
        };
        let mut json_chapters_matched_by_title = false;
        if use_json_chapters {
            let file_stems: Vec<String> = files
                .iter()
                .map(|path| {
                    path.file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or_default()
                        .to_string()
                })
                .collect();
            if let Some(chapters) = json_chapters.take() {
                let (aligned, matched_by_title) =
                    crate::core::books::metadata::align_chapters_to_file_stems(
                        chapters,
                        &file_stems,
                    );
                if matched_by_title {
                    json_chapters_matched_by_title = true;
                    info!(
                        "Aligned metadata.json chapters to local files by title for book_id: {}",
                        book_id
                    );
                }
                json_chapters = Some(aligned);
            }
        }
        let preserve_raw_chapter_titles =
            chapter_title_template_preserves_raw(chapter_title_template);

        // Fetch book to check for regex rule
        let book = self.book_repo.find_by_id(book_id).await?.ok_or_else(|| {
            TingError::not_found(
                LookupErrorKind::Book,
                "process_chapters::not_found",
                "Book not found".to_string(),
            )
        })?;

        let chapter_regex = if let Some(pattern) = &book.chapter_regex {
            regex::Regex::new(pattern).ok()
        } else {
            None
        };

        // Pre-fetch existing chapters to support efficient incremental scanning.
        // Before 1.5.2 local files could be stored as relative paths. Resolve
        // existing files so the old relative path matches the new absolute path.
        let existing_chapters = self.chapter_repo.find_by_book(book_id).await?;

        // Single-file audiobooks (M4B) carry their own chapter table. Probe it
        // when the book has no embedded chapters yet or when the file changed.
        let single_file_mp4 =
            files.len() == 1 && crate::core::audio::mp4::is_mp4_family_extension(&files[0]);
        let embedded_probe_pending = probe_embedded_chapters && single_file_mp4 && {
            let segmented = existing_chapters
                .iter()
                .filter(|chapter| chapter.start_time.is_some())
                .count();
            segmented < existing_chapters.len()
                || changed_file_paths.is_some_and(|paths| paths.contains(&files[0]))
                || last_scanned.is_none()
                || changed_file_paths.is_none()
        };
        if embedded_probe_pending
            && let Some(embedded) = self.extract_embedded_chapters(files).await
        {
            return self
                .process_embedded_chapters(
                    book_id,
                    &files[0],
                    &embedded,
                    ChapterTitleOptions {
                        task_id,
                        template: chapter_title_template,
                        overrides: chapter_title_overrides,
                    },
                )
                .await;
        }

        let mut chapter_map: HashMap<PathBuf, Chapter> = HashMap::new();
        let mut duplicate_chapter_ids = Vec::new();
        for ch in existing_chapters {
            let path = canonical_existing_path(Path::new(&ch.path));
            if let Some(existing) = chapter_map.get(&path) {
                let chapter_is_relative = Path::new(&ch.path).is_relative();
                let existing_is_relative = Path::new(&existing.path).is_relative();
                if chapter_is_relative != existing_is_relative {
                    if chapter_is_relative {
                        duplicate_chapter_ids.push(existing.id.clone());
                        chapter_map.insert(path, ch);
                    } else {
                        duplicate_chapter_ids.push(ch.id);
                    }
                } else {
                    chapter_map.insert(PathBuf::from(&ch.path), ch);
                }
            } else {
                chapter_map.insert(path, ch);
            }
        }
        for chapter_id in duplicate_chapter_ids {
            self.chapter_repo.delete(&chapter_id).await?;
            has_changes = true;
        }

        let mut main_counter = 0;
        let mut extra_counter = 0;

        // Track processed chapter IDs to find deleted ones
        let mut processed_chapter_ids = HashSet::new();

        for (index, file_path) in files.iter().enumerate() {
            if index % 25 == 0 || index + 1 == total_files {
                // Progress/cancellation updates are database operations; keep
                // them periodic instead of writing once per handful of files.
                self.check_cancellation(task_id).await?;
                self.update_progress_key(
                    task_id,
                    "scan.chapter.processing",
                    serde_json::json!({
                        "current": index + 1,
                        "total": total_files,
                    }),
                )
                .await;
            }

            // Incremental Scan Logic
            // Check if file exists in DB
            let canonical_file_path = canonical_existing_path(file_path);
            let mut existing_chapter = chapter_map.get(&canonical_file_path).cloned();

            // Check if file has changed
            let is_modified = if let Some(changed_file_paths) = changed_file_paths {
                changed_file_paths.contains(file_path)
            } else if let Some(last_scan) = last_scanned {
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
            let filename_str = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();
            let ai_chapter_title = chapter_title_overrides
                .and_then(|titles| titles.get(index))
                .map(|title| title.trim())
                .filter(|title| !title.is_empty());
            let mut regex_idx = None;
            let mut regex_title = None;

            if let Some(re) = &chapter_regex
                && let Some(caps) = re.captures(&filename_str)
            {
                if let Some(m) = caps.get(1)
                    && let Ok(idx) = m.as_str().parse::<i32>()
                {
                    regex_idx = Some(idx);
                }
                if let Some(m) = caps.get(2) {
                    regex_title = Some(m.as_str().to_string());
                }
            }

            // Optimization: If chapter exists and file is not modified, skip processing!
            if let Some(ref ch) = existing_chapter
                && !is_modified
            {
                // Update index if needed (e.g. reordering files), but skip hashing/metadata
                // Also respect manual_corrected if we were to update anything else

                let title_override = if ch.manual_corrected == 0 {
                    if let Some(ai_title) = ai_chapter_title {
                        let (_, is_extra) = self
                            .text_cleaner
                            .clean_chapter_title(ai_title, book.title.as_deref());
                        Some((ai_title.to_string(), is_extra))
                    } else if let Some(rt) = regex_title.clone() {
                        let (cleaned, is_extra) = clean_or_preserve_chapter_title(
                            self.text_cleaner.as_ref(),
                            &rt,
                            book.title.as_deref(),
                            preserve_raw_chapter_titles,
                        );
                        Some((cleaned, is_extra))
                    } else {
                        let local_title = json_chapters
                            .as_ref()
                            .filter(|_| use_json_chapters)
                            .and_then(|chapters| chapters.get(index))
                            .map(|chapter| chapter.title.as_str());
                        let (title, source) =
                            scraper_config.select_title(&filename_str, local_title, None, None);
                        let (title, is_extra) = super::super::shared::resolve_chapter_title(
                            self.text_cleaner.as_ref(),
                            title,
                            book.title.as_deref(),
                            None,
                            source != "local_metadata",
                            preserve_raw_chapter_titles,
                            extract_extra_chapters,
                        );
                        (scraper_config.metadata_rank("audio_metadata")
                            >= scraper_config.metadata_rank(source))
                        .then_some((title, is_extra))
                    }
                } else {
                    None
                };

                let counter_is_extra = if ch.manual_corrected != 0 {
                    ch.is_extra == 1
                } else {
                    extract_extra_chapters
                        && title_override
                            .as_ref()
                            .map(|(_, is_extra)| *is_extra)
                            .unwrap_or(ch.is_extra == 1)
                };
                let idx_from_counter = if counter_is_extra {
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
                // 2. The configured title source provides a different title.
                // 3. Index changed due to reordering.

                let mut should_update = false;
                let mut new_title = ch.title.clone();
                let mut new_idx = ch.chapter_index;
                let needs_duration = ch.duration.unwrap_or(0) <= 0
                    && !file_path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("strm"));
                let mut recovered_duration = if json_chapters_matched_by_title || needs_duration {
                    json_chapters
                        .as_ref()
                        .filter(|_| use_json_chapters)
                        .and_then(|chapters| chapters.get(index))
                        .map(|chapter| ((chapter.end - chapter.start).round() as i32).max(0))
                        .filter(|duration| *duration > 0)
                } else {
                    None
                };
                if needs_duration && recovered_duration.is_none() {
                    let (_, _, _, _, _, duration) =
                        self.extract_chapter_metadata(file_path, cloud_mode).await;
                    recovered_duration = (duration > 0).then_some(duration);
                }
                let duration_changed = recovered_duration
                    .map(|duration| ch.duration != Some(duration))
                    .unwrap_or(false);
                let mut new_is_extra = if extract_extra_chapters {
                    ch.is_extra
                } else {
                    0
                };
                if new_is_extra != ch.is_extra {
                    should_update = true;
                }

                // Check Index
                if new_idx != Some(target_idx) {
                    new_idx = Some(target_idx);
                    should_update = true;
                }

                // Check title overrides. metadata.json is a fallback, but chapter regex
                // and forced filename titles must still apply to unchanged files.
                // JSON titles are preserved verbatim, while still detecting extras.
                if let Some((target_title, target_is_extra)) = title_override {
                    let target_title = apply_chapter_title_template(
                        chapter_title_template,
                        book.title.as_deref(),
                        target_idx,
                        &target_title,
                    );

                    if ch.title.as_deref() != Some(&target_title) {
                        new_title = Some(target_title);
                        should_update = true;
                    }

                    let target_is_extra = if extract_extra_chapters && target_is_extra {
                        1
                    } else {
                        0
                    };
                    if new_is_extra != target_is_extra {
                        new_is_extra = target_is_extra;
                        should_update = true;
                    }
                }

                let scanned_path = file_path.to_string_lossy().to_string();
                // A single-file MP4 keeps a whole-file time range so later
                // scans know the embedded chapter table has already been read.
                let marker_duration = recovered_duration.or(ch.duration).unwrap_or(0);
                let (marker_start, marker_end) = segment_bounds(single_file_mp4, marker_duration);
                let marker_changed =
                    single_file_mp4 && (ch.start_time != marker_start || ch.end_time != marker_end);
                if ch.path != scanned_path
                    || duration_changed
                    || marker_changed
                    || (should_update && ch.manual_corrected == 0)
                {
                    let mut updated_ch = ch.clone();
                    updated_ch.path = scanned_path;
                    if let Some(duration) = recovered_duration {
                        updated_ch.duration = Some(duration);
                    }
                    if single_file_mp4 {
                        updated_ch.start_time = marker_start;
                        updated_ch.end_time = marker_end;
                    }
                    if ch.manual_corrected == 0 {
                        updated_ch.chapter_index = new_idx;
                        updated_ch.title = new_title;
                        updated_ch.is_extra = new_is_extra;
                    }
                    self.chapter_repo.update(&updated_ch).await?;
                    has_changes = true;
                }
                processed_chapter_ids.insert(ch.id.clone());
                continue;
            }

            // If we are here, either it's a new file OR it's modified.

            // Calculate content-based hash
            let file_hash = self.calculate_file_hash(file_path)?;

            // Check if chapter exists by Hash (Global Deduplication)
            // But we must be careful: if we already found it by Path, we know it's that chapter.
            // If we found by Path, but Hash changed, it's an update.
            // If we didn't find by Path, we check Hash to see if it's a move/rename.

            if existing_chapter.is_none()
                && let Ok(Some(ch)) = self.chapter_repo.find_by_hash(&file_hash).await
            {
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

            // Zero-length sidecar chapter entries are unknown durations, not a
            // reason to skip probing the audio. Extract once for duration/title.
            let json_duration = json_chapters
                .as_ref()
                .filter(|_| use_json_chapters)
                .and_then(|chapters| chapters.get(index))
                .map(|chapter| ((chapter.end - chapter.start).round() as i32).max(0))
                .filter(|duration| *duration > 0);
            let local_title = json_chapters
                .as_ref()
                .filter(|_| use_json_chapters)
                .and_then(|chapters| chapters.get(index))
                .map(|chapter| chapter.title.as_str());
            let needs_audio_title = scraper_config.metadata_rank("audio_metadata")
                < scraper_config.metadata_rank(
                    scraper_config
                        .select_title(&filename_str, local_title, None, None)
                        .1,
                );
            let extracted = if json_duration.is_none() || needs_audio_title {
                Some(self.extract_chapter_metadata(file_path, cloud_mode).await)
            } else {
                None
            };
            let duration = json_duration
                .or_else(|| extracted.as_ref().map(|metadata| metadata.5))
                .unwrap_or(0);
            let extracted_title = extracted.map(|metadata| metadata.1).unwrap_or_default();

            // Explicit chapter rules override the configured metadata priority.
            let (raw_title, should_clean_title) = if let Some(rt) = regex_title {
                (rt, true)
            } else {
                let (title, source) = scraper_config.select_title(
                    &filename_str,
                    local_title,
                    Some(&extracted_title),
                    None,
                );
                (title.to_owned(), source != "local_metadata")
            };

            let (final_title, is_extra) = super::super::shared::resolve_chapter_title(
                self.text_cleaner.as_ref(),
                &raw_title,
                book.title.as_deref(),
                ai_chapter_title,
                should_clean_title,
                preserve_raw_chapter_titles,
                extract_extra_chapters,
            );

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
            let final_title = apply_chapter_title_template(
                chapter_title_template,
                book.title.as_deref(),
                chapter_idx,
                &final_title,
            );

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
                if single_file_mp4 {
                    let (start, end) = segment_bounds(single_file_mp4, duration);
                    ch.start_time = start;
                    ch.end_time = end;
                }

                self.chapter_repo.update(&ch).await?;
                has_changes = true;
                processed_chapter_ids.insert(ch.id.clone());
            } else {
                // Create New
                let chapter_id = Uuid::new_v4().to_string();
                let (start_time, end_time) = segment_bounds(single_file_mp4, duration);
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
                    start_time,
                    end_time,
                };

                match self.chapter_repo.create(&chapter).await {
                    Ok(_) => {
                        has_changes = true;
                        processed_chapter_ids.insert(chapter_id);
                    }
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

        // A single-file book that lost its embedded chapter table keeps only
        // the whole-file chapter; drop the stale segmented rows.
        if single_file_mp4 {
            let file_path = files[0].to_string_lossy().to_string();
            if let Ok(rows) = self.chapter_repo.find_by_book(book_id).await {
                for row in rows {
                    if row.path == file_path
                        && !processed_chapter_ids.contains(&row.id)
                        && row.manual_corrected == 0
                    {
                        self.chapter_repo.delete(&row.id).await?;
                        has_changes = true;
                    }
                }
            }
        }

        Ok(has_changes)
    }

    /// Read the authored chapter table of a single-file audiobook.
    async fn extract_embedded_chapters(
        &self,
        files: &[PathBuf],
    ) -> Option<Vec<crate::core::audio::metadata::AudioChapter>> {
        let file = files.first()?;
        if !crate::core::audio::mp4::is_mp4_family_extension(file) {
            return None;
        }
        match crate::core::audio::AudioService::read_embedded_chapters(file).await {
            Ok(chapters) if chapters.len() > 1 => Some(chapters),
            Ok(_) => None,
            Err(error) => {
                debug!(
                    path = %file.display(),
                    error = %error,
                    "Could not read embedded chapters; keeping file-based chapters"
                );
                None
            }
        }
    }

    /// Replace the single whole-file chapter with the chapters authored in the
    /// file itself. Titles stay verbatim unless the book defines a template.
    async fn process_embedded_chapters(
        &self,
        book_id: &str,
        file: &Path,
        chapters: &[crate::core::audio::metadata::AudioChapter],
        titles: ChapterTitleOptions<'_>,
    ) -> Result<bool> {
        self.check_cancellation(titles.task_id).await?;
        let book = self.book_repo.find_by_id(book_id).await?.ok_or_else(|| {
            TingError::not_found(
                LookupErrorKind::Book,
                "process_embedded_chapters::not_found",
                "Book not found".to_string(),
            )
        })?;
        let existing = self.chapter_repo.find_by_book(book_id).await?;
        // A manually curated chapter list owns the book; never replace it.
        if existing.iter().any(|chapter| chapter.manual_corrected != 0) {
            info!(book_id = %book_id, "Keeping manually curated chapters");
            return Ok(false);
        }

        let file_string = file.to_string_lossy().to_string();
        let file_key = canonical_existing_path(file);
        let same_file = |path: &str| {
            path == file_string || canonical_existing_path(Path::new(path)) == file_key
        };
        let base_hash = self.calculate_file_hash(file)?;
        let mut processed_ids = HashSet::new();
        let mut has_changes = false;

        for (index, chapter) in chapters.iter().enumerate() {
            let start_ms = chapter.start_ms.max(0);
            let end_ms = chapter.end_ms.max(start_ms + 1);
            let chapter_index = i32::try_from(index + 1).unwrap_or(i32::MAX);
            let raw_title = titles
                .overrides
                .and_then(|overrides| overrides.get(index))
                .map(|title| title.trim())
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| chapter.title.trim());
            let fallback = format!("Chapter {chapter_index}");
            let raw_title = if raw_title.is_empty() {
                fallback.as_str()
            } else {
                raw_title
            };
            let title = apply_chapter_title_template(
                titles.template,
                book.title.as_deref(),
                chapter_index,
                raw_title,
            );
            let duration = (((end_ms - start_ms) as f64) / 1000.0).round().max(1.0) as i32;

            if let Some(mut row) = existing
                .iter()
                .find(|row| same_file(&row.path) && row.start_time == Some(start_ms))
                .cloned()
            {
                let mut changed = false;
                if row.title.as_deref() != Some(title.as_str()) {
                    row.title = Some(title);
                    changed = true;
                }
                if row.chapter_index != Some(chapter_index) {
                    row.chapter_index = Some(chapter_index);
                    changed = true;
                }
                if row.duration != Some(duration) {
                    row.duration = Some(duration);
                    changed = true;
                }
                if row.end_time != Some(end_ms) {
                    row.end_time = Some(end_ms);
                    changed = true;
                }
                let hash = segment_hash(&base_hash, start_ms);
                if row.hash.as_deref() != Some(hash.as_str()) {
                    row.hash = Some(hash);
                    changed = true;
                }
                if changed {
                    self.chapter_repo.update(&row).await?;
                    has_changes = true;
                }
                processed_ids.insert(row.id);
            } else {
                let chapter_id = Uuid::new_v4().to_string();
                let row = Chapter {
                    id: chapter_id.clone(),
                    book_id: book_id.to_string(),
                    title: Some(title),
                    path: file_string.clone(),
                    duration: Some(duration),
                    chapter_index: Some(chapter_index),
                    is_extra: 0,
                    hash: Some(segment_hash(&base_hash, start_ms)),
                    manual_corrected: 0,
                    start_time: Some(start_ms),
                    end_time: Some(end_ms),
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                self.chapter_repo.create(&row).await?;
                processed_ids.insert(chapter_id);
                has_changes = true;
            }
        }

        // Drop the previous whole-file chapter and any chapter that no longer
        // exists in the file's chapter table.
        for row in existing {
            if same_file(&row.path) && !processed_ids.contains(&row.id) {
                self.chapter_repo.delete(&row.id).await?;
                has_changes = true;
            }
        }

        info!(
            book_id = %book_id,
            chapters = chapters.len(),
            "Applied embedded audio chapters"
        );
        Ok(has_changes)
    }

    fn calculate_file_hash(&self, path: &Path) -> Result<String> {
        let mut file = std::fs::File::open(path).map_err(TingError::IoError)?;
        let metadata = file.metadata().map_err(TingError::IoError)?;
        let len = metadata.len();

        let mut buffer = vec![0; 16384]; // 16KB
        let n = file.read(&mut buffer).map_err(TingError::IoError)?;

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
fn canonical_existing_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

struct ChapterTitleOptions<'a> {
    task_id: Option<&'a str>,
    template: Option<&'a str>,
    overrides: Option<&'a [String]>,
}

/// Time range stored on the chapter of a single-file audiobook. Both bounds
/// stay empty for books that use one file per chapter.
fn segment_bounds(single_file: bool, duration: i32) -> (Option<i64>, Option<i64>) {
    if !single_file {
        return (None, None);
    }
    let end = (duration > 0).then(|| i64::from(duration) * 1000);
    (Some(0), end)
}

fn segment_hash(base_hash: &str, start_ms: i64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(base_hash.as_bytes());
    hasher.update(b"#");
    hasher.update(start_ms.to_le_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
#[path = "../../../../tests/unit/core/library_scanner/local/chapters.rs"]
mod tests;
