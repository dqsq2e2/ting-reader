//! Reconcile one local book, its chapters and metadata outputs.

use super::*;

impl LibraryScanner {
    /// Process a directory containing audio files as a book
    pub(super) async fn process_book_directory(
        &self,
        context: LocalBookProcessingContext<'_>,
    ) -> Result<(String, ScanStatus)> {
        let LocalBookProcessingContext {
            library_id,
            dir,
            files,
            last_scanned,
            task_id,
            scraper_config,
            manual_corrected_patterns,
            existing_info,
            chapter_counts,
            changed_file_paths,
            fallback_title_override,
            mode,
        } = context;
        // Log scraper config for debugging
        debug!(
            "Processing book dir: {:?}, nfo_enabled: {}, json_enabled: {}",
            dir, scraper_config.nfo_writing_enabled, scraper_config.metadata_writing_enabled
        );

        // 0. Check New Chapter Protection only for a directory that is not
        // already mapped to a known book. This avoids testing every lock regex
        // against every existing directory during an incremental scan.
        if existing_info.is_none() {
            for (book_id, pattern) in manual_corrected_patterns {
                if !pattern.is_empty() {
                    let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if let Ok(re) = regex::Regex::new(pattern)
                        && re.is_match(dir_name)
                    {
                        info!(
                            "New Chapter Protection: Merging {} into existing book {}",
                            dir_name, book_id
                        );
                        let has_changes = self
                            .process_chapters(
                                book_id,
                                files,
                                ChapterProcessingOptions {
                                    last_scanned: if mode.is_full() { None } else { last_scanned },
                                    task_id,
                                    scraper_config,
                                    changed_file_paths: (!mode.is_full())
                                        .then_some(changed_file_paths),
                                    json_chapters: None,
                                    chapter_title_template: None,
                                    chapter_title_overrides: None,
                                    probe_embedded_chapters: false,
                                },
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

        // 2. Incremental fast path. Audio and sidecar mtimes are checked once,
        // and an unchanged directory returns before chapter reads or scraper I/O.
        let max_mtime = files
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok().and_then(|m| m.modified().ok()))
            .chain(
                [dir.join("metadata.json"), dir.join("book.nfo")]
                    .iter()
                    .filter_map(|path| {
                        std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
                    }),
            )
            .max();
        let max_mtime_utc = max_mtime.map(chrono::DateTime::<chrono::Utc>::from);

        // A locked book is intentionally never auto-repaired by writing sidecars.
        // Do not defeat the incremental fast path just because a locked book
        // lacks a file that the current write setting would otherwise create.
        let required_sidecar_missing = !is_manual_corrected
            && ((scraper_config.metadata_writing_enabled && !dir.join("metadata.json").exists())
                || (scraper_config.nfo_writing_enabled && !dir.join("book.nfo").exists()));
        let mut skip_metadata_update = false;
        if !mode.is_full() {
            if let (Some(last_scan), Some(max_mt)) = (last_scanned, max_mtime_utc)
                && max_mt <= last_scan
                && !required_sidecar_missing
                && let Some(book_id) = existing_book_id.as_deref()
            {
                skip_metadata_update = chapter_counts
                    .get(book_id)
                    .map(|counts| {
                        counts.path_count == files.len()
                            && counts.missing_duration == 0
                            && !embedded_chapters_pending(files, counts)
                    })
                    .unwrap_or(false);
            }

            if let Some(book_id) = existing_book_id.clone().filter(|_| skip_metadata_update) {
                return Ok((book_id, ScanStatus::Skipped));
            }
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
        if let Some(fallback_title) = fallback_title_override
            && !fallback_title.trim().is_empty()
            && source == MetadataSource::Fallback
        {
            title = self.text_cleaner.clean_book_directory_title(fallback_title);
        }
        let mut author = scanned_meta.author;
        let mut narrator = scanned_meta.narrator;
        let mut description = scanned_meta.description;
        let mut tags = scanned_meta.tags;
        let mut genre = scanned_meta.genre;
        let mut cover_url = scanned_meta.cover_url;

        // Extended fields
        let subtitle = scanned_meta.subtitle;
        let mut published_year = scanned_meta.published_year;
        let published_date = scanned_meta.published_date;
        let publisher = scanned_meta.publisher;
        let isbn = scanned_meta.isbn;
        let asin = scanned_meta.asin;
        let language = scanned_meta.language;
        let explicit = scanned_meta.explicit;
        let abridged = scanned_meta.abridged;
        let json_series = scanned_meta.json_series;
        let json_chapters = scanned_meta.json_chapters;
        let chapter_title_template = scanned_meta.chapter_title_template;
        let chapter_titles = scanned_meta.chapter_titles;

        // 3. Apply Manual Correction or Existing Data
        if is_manual_corrected
            && let Some(id) = &existing_book_id
            && let Ok(Some(book)) = self.book_repo.find_by_id(id).await
        {
            // A metadata lock owns the complete editable metadata shape,
            // including intentionally empty fields. Preserve those values
            // instead of filling only missing fields from a later scan.
            title = book.title.unwrap_or(title);
            author = book.author;
            narrator = book.narrator;
            description = book.description;
            tags = book.tags;
            genre = book.genre;
            cover_url = book.cover_url;
            // `year` is the persisted form of published_year in the
            // current book model, so preserve an explicit empty value.
            published_year = book.year.map(|year| year.to_string());
            // theme_color will be recalculated if cover_url changed later
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

            if let Ok(Some(color)) = crate::core::books::color::calculate_theme_color_with_client(
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
            // Chapter parsing rules are user-owned too; never drop a saved rule
            // merely because a scan source did not provide one.
            if book.chapter_regex.is_none() && existing.chapter_regex.is_some() {
                book.chapter_regex = existing.chapter_regex.clone();
            }
            if existing.manual_corrected == 0 {
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
                ChapterProcessingOptions {
                    last_scanned: if mode.is_full() { None } else { last_scanned },
                    task_id,
                    scraper_config,
                    changed_file_paths: (!mode.is_full()).then_some(changed_file_paths),
                    json_chapters,
                    chapter_title_template: chapter_title_template.as_deref(),
                    chapter_title_overrides: if chapter_titles.is_empty() {
                        None
                    } else {
                        Some(chapter_titles.as_slice())
                    },
                    probe_embedded_chapters: true,
                },
            )
            .await?;

        self.metadata_service()
            .import_series(&book, &json_series)
            .await?;

        // Manual ownership blocks automatic sidecar writes while chapter discovery continues.
        if !is_manual_corrected {
            let patch = crate::core::books::metadata::MetadataPatch {
                subtitle,
                published_year,
                published_date,
                publisher,
                isbn,
                asin,
                language,
                explicit: Some(explicit),
                abridged: Some(abridged),
                duration: None,
            };
            if let Err(error) = self
                .metadata_service()
                .sync(
                    &book,
                    crate::core::books::metadata::MetadataChange::Book,
                    &patch,
                )
                .await
            {
                warn!(book_id = %book_id, error = %error, "Failed to synchronize scanned book metadata");
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
                if path.is_file()
                    && let Some(ext) = path.extension()
                {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if ["jpg", "jpeg", "png", "webp"].contains(&ext_str.as_str()) {
                        // Return path with forward slashes for better JSON/URL compatibility
                        return Some(path.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }
        None
    }

    pub(super) fn generate_book_hash(&self, audiobook_dir: &Path) -> String {
        let path_str = audiobook_dir.to_string_lossy();
        let mut hasher = Sha256::new();
        hasher.update(path_str.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// A single-file MP4 whose chapter table has not been read yet must be probed
/// once even when the file itself is unchanged (for example after upgrading
/// from a version that stored one chapter per file).
pub(super) fn embedded_chapters_pending(
    files: &[PathBuf],
    counts: &crate::db::repository::chapter::ChapterCounts,
) -> bool {
    files.len() == 1
        && crate::core::audio::mp4::is_mp4_family_extension(&files[0])
        && counts.segmented < counts.total
}
