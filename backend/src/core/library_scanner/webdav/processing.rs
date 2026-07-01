use super::super::shared::{
    apply_chapter_title_template, chapter_title_template_preserves_raw,
    clean_or_preserve_chapter_title,
};
use super::super::{LibraryScanner, MetadataSource, ScanStatus};
use crate::core::error::Result;
use crate::core::nfo_manager::BookMetadata;
use crate::db::repository::Repository;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::{info, warn};
use uuid::Uuid;

impl LibraryScanner {
    pub(super) async fn process_webdav_book(
        &self,
        library: &crate::db::models::Library,
        dir_url: &str,
        file_urls: &[String],
        metadata_files: &[String],
        _task_id: Option<&str>,
        scraper_config: &crate::db::models::ScraperConfig,
        existing_info: Option<(String, i32, Option<String>)>,
        fallback_title_override: Option<&str>,
    ) -> Result<(String, ScanStatus)> {
        // Derive title from directory name
        // Decode URL to handle percent-encoded characters (e.g. Chinese)
        let decoded_url = self.decode_url_path(dir_url);
        let dir_name_title = decoded_url
            .trim_end_matches('/')
            .split('/')
            .last()
            .unwrap_or("Unknown Book")
            .to_string();
        let (cleaned_dir_name, _) = self.text_cleaner.clean_chapter_title(&dir_name_title, None);

        // No local path, use URL as path
        // We use the original URL as path to ensure connectivity, but StorageService needs to handle it correctly
        let path = dir_url.to_string();

        // Check if book exists
        let mut hasher = Sha256::new();
        hasher.update(path.as_bytes());
        let path_hash = format!("{:x}", hasher.finalize());
        let book_hash = path_hash.clone();

        // Prepare temp directory for WebDAV book metadata and cover
        // Structure: temp/{book_hash}/
        let temp_book_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("temp")
            .join(&book_hash);
        if !temp_book_dir.exists() {
            std::fs::create_dir_all(&temp_book_dir).ok();
        }

        // Extended metadata fields for WebDAV (to be written to metadata.json)
        let mut subtitle: Option<String> = None;
        let mut published_year: Option<String> = None;
        let mut published_date: Option<String> = None;
        let mut publisher: Option<String> = None;
        let mut isbn: Option<String> = None;
        let mut asin: Option<String> = None;
        let mut language: Option<String> = None;
        let mut explicit: bool = false;
        let mut abridged: bool = false;
        let mut json_tags: Vec<String> = Vec::new();
        let mut json_series: Vec<String> = Vec::new();
        let mut chapter_title_template: Option<String> = None;
        let mut ai_chapter_titles: Vec<String> = Vec::new();
        let is_cloud_mode = scraper_config.cloud_mode;

        // Extract metadata from WebDAV files (try multiple files if needed)
        let (
            mut meta_album,
            mut _meta_chapter_title,
            mut meta_author,
            mut meta_narrator,
            mut meta_cover_url,
            _meta_duration,
        ) = if !is_cloud_mode && !file_urls.is_empty() {
            // 尝试多个文件，直到找到完整的元数据（包括封面）
            let mut album = String::new();
            let mut title = String::new();
            let mut author = None;
            let mut narrator = None;
            let mut cover_url = None;
            let mut duration = 0;

            let max_files = std::cmp::min(file_urls.len(), 3); // 最多尝试 3 个文件

            for (index, file_url) in file_urls.iter().take(max_files).enumerate() {
                let (a, t, au, n, c, d) = self
                    .extract_webdav_metadata(
                        library,
                        file_url,
                        if index == 0 {
                            Some(&temp_book_dir)
                        } else {
                            None
                        }, // 只在第一个文件时保存封面到目录
                        scraper_config.extract_audio_cover,
                    )
                    .await;

                if index == 0 {
                    tracing::debug!("Extracting metadata from the first WebDAV file");
                } else {
                    tracing::debug!("Supplementing metadata from WebDAV file #{}", index + 1);
                }

                // 只在第一个文件或缺失时提取基本元数据
                if index == 0 || album.is_empty() {
                    if !a.is_empty() {
                        album = a;
                    }
                }
                if index == 0 || title.is_empty() {
                    if !t.is_empty() {
                        title = t;
                    }
                }
                if index == 0 || author.is_none() {
                    if au.is_some() {
                        author = au;
                    }
                }
                if index == 0 || narrator.is_none() {
                    if n.is_some() {
                        narrator = n;
                    }
                }
                if index == 0 {
                    duration = d;
                }

                // 封面：如果还没有找到，继续尝试
                if scraper_config.extract_audio_cover && cover_url.is_none() {
                    if c.is_some() {
                        cover_url = c;
                        if index > 0 {
                            tracing::info!("Found cover in WebDAV file #{}", index + 1);
                        }
                    }
                }

                // 如果已经找到了所有需要的元数据，就停止
                let has_basic_metadata = !album.is_empty() || author.is_some();
                let has_cover_if_needed =
                    !scraper_config.extract_audio_cover || cover_url.is_some();

                if has_basic_metadata && has_cover_if_needed {
                    tracing::debug!("Complete WebDAV metadata found; stopping file attempts");
                    break;
                }
            }

            (album, title, author, narrator, cover_url, duration)
        } else {
            (String::new(), String::new(), None, None, None, 0)
        };

        // Try to fetch and parse metadata.json and book.nfo from WebDAV
        // We do this by downloading them to temp_book_dir
        if let Some(storage) = &self.storage_service {
            // Decryption key
            let key = self.encryption_key.as_deref().unwrap_or(&[0u8; 32]);

            for meta_url in metadata_files {
                let filename = meta_url.split('/').last().unwrap_or_default();
                if filename == "metadata.json" || filename == "book.nfo" {
                    let temp_path = temp_book_dir.join(filename);
                    if let Ok((mut reader, _)) = storage
                        .get_webdav_reader(library, meta_url, None, key)
                        .await
                    {
                        if let Ok(mut file) = tokio::fs::File::create(&temp_path).await {
                            let _ = tokio::io::copy(&mut reader, &mut file).await;
                        }
                    }
                }
            }
        }

        // Read metadata.json if downloaded
        let mut json_chapters: Option<Vec<crate::core::metadata_writer::AudiobookshelfChapter>> =
            None;
        if let Ok(Some(json_meta)) =
            crate::core::metadata_writer::read_metadata_json(&temp_book_dir)
        {
            if let Some(t) = json_meta.title {
                meta_album = t;
            }
            if !json_meta.authors.is_empty() {
                meta_author = Some(json_meta.authors[0].clone());
            }
            if !json_meta.narrators.is_empty() {
                meta_narrator = Some(json_meta.narrators[0].clone());
            }
            if !json_meta.series.is_empty() {
                json_series = json_meta.series;
            }
            if !json_meta.tags.is_empty() {
                json_tags = json_meta.tags;
            }

            // Store chapters for later use
            if !json_meta.chapters.is_empty() {
                json_chapters = Some(json_meta.chapters);
            }

            // Extended
            subtitle = json_meta.subtitle;
            published_year = json_meta.published_year;
            published_date = json_meta.published_date;
            publisher = json_meta.publisher;
            isbn = json_meta.isbn;
            asin = json_meta.asin;
            language = json_meta.language;
            explicit = json_meta.explicit;
            abridged = json_meta.abridged;
        }

        // Read book.nfo if downloaded (merge, lower priority than json usually, but let's check)
        // If metadata.json was present, we prefer it.
        // If not, we check nfo.
        let nfo_path = temp_book_dir.join("book.nfo");
        if nfo_path.exists() {
            if let Ok(nfo_meta) = self.nfo_manager.read_book_nfo(&nfo_path) {
                if meta_album.is_empty() && !nfo_meta.title.is_empty() {
                    meta_album = nfo_meta.title;
                }
                if meta_author.is_none() && !nfo_meta.author.is_none() {
                    meta_author = nfo_meta.author;
                }
                if meta_narrator.is_none() && !nfo_meta.narrator.is_none() {
                    meta_narrator = nfo_meta.narrator;
                }
                if meta_cover_url.is_none() && !nfo_meta.cover_url.is_none() {
                    meta_cover_url = nfo_meta.cover_url;
                }
            }
        }

        // Also check if there's a local cover image directly in the webdav folder
        // Download it to temp_book_dir so the proxy can serve it as a local file.
        // Storing the raw WebDAV URL doesn't work because the frontend/proxy lacks WebDAV auth.
        if meta_cover_url.is_none() {
            if let Some(storage) = &self.storage_service {
                let key = self.encryption_key.as_deref().unwrap_or(&[0u8; 32]);
                for meta_url in metadata_files {
                    let filename = meta_url
                        .split('/')
                        .last()
                        .unwrap_or_default()
                        .to_lowercase();
                    if [
                        "cover.jpg",
                        "cover.png",
                        "cover.jpeg",
                        "cover.webp",
                        "folder.jpg",
                    ]
                    .contains(&filename.as_str())
                    {
                        // Download cover image to temp_book_dir
                        let original_ext = meta_url.split('.').last().unwrap_or("jpg");
                        let temp_cover_path = temp_book_dir.join(format!("cover.{}", original_ext));
                        if !temp_cover_path.exists() {
                            if let Ok((mut reader, _)) = storage
                                .get_webdav_reader(library, meta_url, None, key)
                                .await
                            {
                                if let Ok(mut file) =
                                    tokio::fs::File::create(&temp_cover_path).await
                                {
                                    let _ = tokio::io::copy(&mut reader, &mut file).await;
                                    tracing::debug!(
                                        "Downloaded WebDAV cover to {:?}",
                                        temp_cover_path
                                    );
                                }
                            }
                        }
                        if temp_cover_path.exists() {
                            meta_cover_url =
                                Some(temp_cover_path.to_string_lossy().replace('\\', "/"));
                        }
                        break;
                    }
                }
            }
        }

        let mut book_title;
        let source;

        // Title Selection Logic: Priority Local Metadata > ID3 > Fallback
        if scraper_config.use_filename_as_title {
            book_title = fallback_title_override
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(&cleaned_dir_name)
                .to_string();
            source = MetadataSource::Fallback;
        } else if !meta_album.trim().is_empty() && !meta_album.to_lowercase().starts_with("track") {
            // Priority 1/2: metadata.json or ID3 (already merged above)
            // Bugfix: Ignore generic "Track XX" titles from ID3 metadata
            book_title = meta_album.clone();
            source = MetadataSource::FileMetadata;
        } else {
            book_title = fallback_title_override
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(&cleaned_dir_name)
                .to_string();
            source = MetadataSource::Fallback;
        }

        // Clean the book title (whether from ID3 or Directory)
        book_title = self.text_cleaner.clean_filename(&book_title);

        let (book_id, manual_corrected) = if let Some((ref id, mc, _)) = existing_info {
            (id.clone(), mc == 1)
        } else if let Ok(Some(book)) = self.book_repo.find_by_hash(&path_hash).await {
            (book.id, book.manual_corrected == 1)
        } else {
            (Uuid::new_v4().to_string(), false)
        };

        // Create or Update book
        let mut book = crate::db::models::Book {
            id: book_id.clone(),
            library_id: library.id.clone(),
            title: Some(book_title.clone()),
            author: meta_author.or(Some("Unknown".to_string())),
            narrator: meta_narrator,
            cover_url: meta_cover_url,
            description: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            path: path.clone(),
            hash: path_hash.clone(),
            theme_color: None,
            skip_intro: 0,
            skip_outro: 0,
            tags: None,
            genre: None,
            year: published_year.as_ref().and_then(|y| y.parse::<i32>().ok()),
            manual_corrected: if manual_corrected { 1 } else { 0 },
            match_pattern: None,
            chapter_regex: None,
        };

        // If manual corrected, we should preserve existing fields.
        // We need to fetch the existing book to do that properly if we are updating.
        if manual_corrected {
            if let Ok(Some(existing_book)) = self.book_repo.find_by_id(&book_id).await {
                book.title = existing_book.title;
                book.author = existing_book.author;
                book.narrator = existing_book.narrator;
                book.description = existing_book.description;
                book.tags = existing_book.tags;
                book.cover_url = existing_book.cover_url;
                book.theme_color = existing_book.theme_color;
                book.chapter_regex = existing_book.chapter_regex;
            }
        }

        // Run scraper if enabled and NOT manual corrected
        if !manual_corrected {
            if let Some(scraper_service) = &self.scraper_service {
                let chapter_candidates = file_urls
                    .iter()
                    .enumerate()
                    .map(|(index, file_url)| {
                        let decoded_file_url = self.decode_url_path(file_url);
                        let filename = decoded_file_url
                            .split('/')
                            .last()
                            .unwrap_or("chapter")
                            .to_string();
                        let title = filename
                            .rsplit_once('.')
                            .map(|(stem, _)| stem)
                            .filter(|stem| !stem.is_empty())
                            .unwrap_or(&filename)
                            .to_string();

                        serde_json::json!({
                            "index": index + 1,
                            "filename": filename,
                            "title": title,
                            "path": file_url,
                        })
                    })
                    .collect::<Vec<_>>();
                let scrape_context = serde_json::json!({
                    "library_type": "webdav",
                    "directory": dir_url,
                    "directory_name": dir_name_title,
                    "chapters": chapter_candidates,
                    "current_metadata": {
                        "title": book.title,
                        "author": book.author,
                        "narrator": book.narrator,
                        "cover_url": book.cover_url,
                        "description": book.description,
                        "tags": book.tags,
                    },
                });
                match scraper_service
                    .scrape_book_metadata_with_context(
                        &book_title,
                        scraper_config,
                        Some(scrape_context),
                    )
                    .await
                {
                    Ok(detail) => {
                        if !detail.title.is_empty() {
                            // Overwrite if ID3 is empty OR if we are using Fallback source (Directory Name)
                            // Requirement: "If using directory name as book name, then scraped data > ID3 data"
                            if source == MetadataSource::Fallback || meta_album.trim().is_empty() {
                                book.title = Some(detail.title);
                            }
                        }

                        if !detail.author.is_empty() {
                            // Overwrite if Fallback source (Directory Name) OR if current is Unknown/None
                            if source == MetadataSource::Fallback
                                || book.author.as_deref() == Some("Unknown")
                                || book.author.is_none()
                            {
                                book.author = Some(detail.author);
                            }
                        }

                        if !detail.intro.is_empty() {
                            if source == MetadataSource::Fallback || book.description.is_none() {
                                book.description = Some(detail.intro);
                            }
                        }

                        if detail.cover_url.is_some() {
                            if source == MetadataSource::Fallback || book.cover_url.is_none() {
                                book.cover_url = detail.cover_url;
                            }
                        }

                        if detail.narrator.is_some() {
                            if source == MetadataSource::Fallback || book.narrator.is_none() {
                                book.narrator = detail.narrator;
                            }
                        }

                        if !detail.tags.is_empty() {
                            if source == MetadataSource::Fallback || book.tags.is_none() {
                                book.tags = Some(detail.tags.join(","));
                            }
                        }

                        // Capture extended metadata for metadata.json
                        if detail.subtitle.is_some() {
                            subtitle = detail.subtitle;
                        }
                        if detail.published_year.is_some() {
                            published_year = detail.published_year;
                        }
                        if detail.published_date.is_some() {
                            published_date = detail.published_date;
                        }
                        if detail.publisher.is_some() {
                            publisher = detail.publisher;
                        }
                        if detail.isbn.is_some() {
                            isbn = detail.isbn;
                        }
                        if detail.asin.is_some() {
                            asin = detail.asin;
                        }
                        if detail.language.is_some() {
                            language = detail.language;
                        }
                        if detail.explicit {
                            explicit = true;
                        }
                        if detail.abridged {
                            abridged = true;
                        }
                        chapter_title_template = detail.chapter_title_template;
                        if !detail.chapter_titles.is_empty() {
                            ai_chapter_titles = detail.chapter_titles;
                        }
                    }
                    Err(e) => {
                        warn!("Scraper failed for WebDAV book {}: {}", book_title, e);
                    }
                }
            }
        }

        // Calculate theme color if cover exists
        // If cover is from scraper (http), we fetch it.
        // If cover is local (relative), we fetch it from WebDAV.
        // Currently scraper returns HTTP URLs usually.
        // But if we want to support cover.jpg in WebDAV folder:
        // We need to implement find_cover_image for WebDAV.

        // For now, if scraper provided cover_url, we try to calculate color.
        if !manual_corrected {
            if let Some(ref url) = book.cover_url {
                let cover_path = if url.starts_with("//") {
                    format!("https:{}", url)
                } else {
                    url.clone()
                };
                if let Ok(Some(color)) = crate::core::color::calculate_theme_color_with_client(
                    &cover_path,
                    &self.http_client,
                )
                .await
                {
                    book.theme_color = Some(color);
                }
            }
        }

        let mut status = ScanStatus::Created;
        // Check if existing book (by ID check above)
        if existing_info.is_some() {
            if !manual_corrected {
                // Preserve chapter_regex from existing book if not set in metadata
                if book.chapter_regex.is_none() {
                    if let Ok(Some(existing)) = self.book_repo.find_by_id(&book_id).await {
                        book.chapter_regex = existing.chapter_regex;
                    }
                }
                self.book_repo.update(&book).await?;
                status = ScanStatus::Updated;
            } else {
                status = ScanStatus::Skipped;
            }
        } else if let Ok(Some(existing)) = self.book_repo.find_by_id(&book_id).await {
            if !manual_corrected {
                // Preserve chapter_regex from existing book if not set in metadata
                if book.chapter_regex.is_none() {
                    book.chapter_regex = existing.chapter_regex;
                }
                self.book_repo.update(&book).await?;
                status = ScanStatus::Updated;
            } else {
                status = ScanStatus::Skipped;
            }
        } else {
            self.book_repo.create(&book).await?;
        }

        // Create chapters
        let mut main_counter = 0;
        let mut extra_counter = 0;

        // Fetch book to check for regex rule
        let regex_pattern = if manual_corrected {
            self.book_repo
                .find_by_id(&book_id)
                .await?
                .and_then(|b| b.chapter_regex)
        } else {
            book.chapter_regex.clone()
        };

        let chapter_regex = regex_pattern.and_then(|p| regex::Regex::new(&p).ok());

        // Track processed chapter IDs to find deleted ones
        let mut processed_chapter_ids = HashSet::new();
        let mut chapters_changed = false;

        // Check if we can use JSON chapters
        let use_json_chapters = if let Some(ref chapters) = json_chapters {
            if chapters.len() == file_urls.len() {
                info!(
                    "Using metadata.json chapters for WebDAV book: {}",
                    book_title
                );
                true
            } else {
                if !chapters.is_empty() {
                    warn!("metadata.json chapter count ({}) does not match file count ({}) for WebDAV book {}. Ignoring JSON chapters.",
                          chapters.len(), file_urls.len(), book_title);
                }
                false
            }
        } else {
            false
        };
        let preserve_raw_chapter_titles =
            chapter_title_template_preserves_raw(chapter_title_template.as_deref());

        for (index, file_url) in file_urls.iter().enumerate() {
            // Decode filename for title
            let decoded_file_url = self.decode_url_path(file_url);
            let filename = decoded_file_url
                .split('/')
                .last()
                .unwrap_or("chapter")
                .to_string();
            let ai_chapter_title = ai_chapter_titles
                .get(index)
                .map(|title| title.trim())
                .filter(|title| !title.is_empty());

            // Regex extraction
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

            // Check if chapter exists to avoid duplicates
            let mut ch_hasher = Sha256::new();
            ch_hasher.update(file_url.as_bytes());
            let ch_hash = format!("{:x}", ch_hasher.finalize());

            // Extract metadata from WebDAV file (download header chunk)
            // In cloud mode we avoid probing WebDAV audio files and rely solely on scraped/sidecar metadata
            let (meta_title, meta_duration) = if use_json_chapters {
                if let Some(ref chapters) = json_chapters {
                    if index < chapters.len() {
                        let chapter = &chapters[index];
                        let duration = ((chapter.end - chapter.start).round() as i32).max(0);
                        (chapter.title.clone(), duration)
                    } else if is_cloud_mode {
                        // Cloud mode: fallback to filename only, no remote probing
                        (filename.clone(), 0)
                    } else {
                        // Fallback (should not happen)
                        let (_, t, _, _, _, d) = self
                            .extract_webdav_metadata(
                                library,
                                file_url,
                                None,
                                scraper_config.extract_audio_cover,
                            )
                            .await;
                        (t, d)
                    }
                } else if is_cloud_mode {
                    (filename.clone(), 0)
                } else {
                    // Fallback (should not happen)
                    let (_, t, _, _, _, d) = self
                        .extract_webdav_metadata(
                            library,
                            file_url,
                            None,
                            scraper_config.extract_audio_cover,
                        )
                        .await;
                    (t, d)
                }
            } else if is_cloud_mode {
                // Cloud/WebDAV mode without JSON chapters: use filename as title and duration 0
                (filename.clone(), 0)
            } else {
                // Extract from WebDAV file
                let (_, t, _, _, _, d) = self
                    .extract_webdav_metadata(
                        library,
                        file_url,
                        None,
                        scraper_config.extract_audio_cover,
                    )
                    .await;
                (t, d)
            };

            // metadata.json is a fallback title source. Explicit chapter regex
            // and filename-based titles still override it and go through cleaner.
            let (raw_title, should_clean_title) = if let Some(rt) = regex_title {
                (rt, true)
            } else if scraper_config.use_filename_as_title {
                (filename.clone(), true)
            } else if use_json_chapters {
                (meta_title, false)
            } else if !meta_title.trim().is_empty()
                && !meta_title.to_lowercase().starts_with("track")
            {
                (meta_title, true)
            } else {
                (filename, true)
            };

            let (final_title, is_extra) = if let Some(ai_title) = ai_chapter_title {
                let (_, is_extra) = self
                    .text_cleaner
                    .clean_chapter_title(ai_title, book.title.as_deref());
                (ai_title.to_string(), is_extra)
            } else if should_clean_title {
                clean_or_preserve_chapter_title(
                    self.text_cleaner.as_ref(),
                    &raw_title,
                    book.title.as_deref(),
                    preserve_raw_chapter_titles,
                )
            } else {
                let (_, is_extra) = self
                    .text_cleaner
                    .clean_chapter_title(&raw_title, book.title.as_deref());
                (raw_title, is_extra)
            };

            let counter_idx = if is_extra {
                extra_counter += 1;
                extra_counter
            } else {
                main_counter += 1;
                main_counter
            };

            let chapter_idx = regex_idx.unwrap_or(counter_idx);
            let final_title = apply_chapter_title_template(
                chapter_title_template.as_deref(),
                book.title.as_deref(),
                chapter_idx,
                &final_title,
            );

            let chapter = crate::db::models::Chapter {
                id: Uuid::new_v4().to_string(),
                book_id: book_id.clone(),
                title: Some(final_title),
                path: file_url.clone(),
                duration: Some(meta_duration),
                chapter_index: Some(chapter_idx),
                is_extra: if is_extra { 1 } else { 0 },
                hash: Some(ch_hash.clone()),
                created_at: chrono::Utc::now().to_rfc3339(),
                manual_corrected: 0,
            };

            // Check if chapter exists by hash (Deduplication)
            if let Ok(Some(mut existing)) = self.chapter_repo.find_by_hash(&ch_hash).await {
                // Update existing chapter
                // Check Lock
                if existing.manual_corrected == 0 {
                    existing.title = chapter.title;
                    existing.chapter_index = chapter.chapter_index;
                    existing.is_extra = chapter.is_extra;
                }
                existing.duration = chapter.duration;
                existing.book_id = book_id.clone(); // Ensure it belongs to this book
                self.chapter_repo.update(&existing).await?;
                processed_chapter_ids.insert(existing.id.clone());
                chapters_changed = true;
            } else {
                self.chapter_repo.create(&chapter).await?;
                processed_chapter_ids.insert(chapter.id.clone());
                chapters_changed = true;
            }
        }

        // Handle deleted chapters
        if let Ok(existing_chapters) = self.chapter_repo.find_by_book(&book_id).await {
            for ch in existing_chapters {
                if !processed_chapter_ids.contains(&ch.id) {
                    info!("Removing missing chapter from DB: {:?}", ch.path);
                    if let Err(e) = self.chapter_repo.delete(&ch.id).await {
                        warn!("Failed to delete missing chapter {}: {}", ch.id, e);
                    } else {
                        chapters_changed = true;
                    }
                }
            }
        }

        // Process Series
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
                    library_id: library.id.clone(),
                    title: series_title.clone(),
                    author: book.author.clone(), // Initial author from first found book
                    narrator: book.narrator.clone(),
                    cover_url: book.cover_url.clone(),
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

        // Fetch all chapters to generate metadata.json correctly with cumulative times
        let chapters = self.chapter_repo.find_by_book(&book_id).await?;
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

        // Write metadata.json to temp dir for WebDAV book
        if scraper_config.metadata_writing_enabled {
            // Try to preserve existing tags from temp dir if metadata.json exists
            if let Ok(Some(existing_meta)) =
                crate::core::metadata_writer::read_metadata_json(&temp_book_dir)
            {
                if !existing_meta.tags.is_empty() {
                    json_tags = existing_meta.tags;
                }
            }

            let extended_meta = crate::core::metadata_writer::ExtendedMetadata {
                subtitle: subtitle.clone(),
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

            if let Err(e) =
                crate::core::metadata_writer::write_metadata_json(&temp_book_dir, &metadata_json)
            {
                warn!(
                    "Failed to write metadata.json for WebDAV book {}: {}",
                    book_title, e
                );
            }
        }

        // Write NFO to temp dir for WebDAV book
        if scraper_config.nfo_writing_enabled {
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
            metadata.subtitle = subtitle; // Pass subtitle to NFO if available

            if let Some(tags_str) = &book.tags {
                metadata.tags.items = tags_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }

            if let Err(e) = self
                .nfo_manager
                .write_book_nfo_to_dir(&temp_book_dir, &metadata)
            {
                warn!(
                    "Failed to write NFO for WebDAV book {}: {}",
                    book.title.unwrap_or_default(),
                    e
                );
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
}
