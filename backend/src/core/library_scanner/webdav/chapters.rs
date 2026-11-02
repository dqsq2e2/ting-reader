//! Remote chapter reconciliation, title policy and deletion scope.

use super::super::LibraryScanner;
use super::super::shared::{apply_chapter_title_template, chapter_title_template_preserves_raw};
use crate::core::app::error::Result;
use crate::db::models::{Book, Library, ScraperConfig};
use crate::db::repository::Repository;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use tracing::{info, warn};
use uuid::Uuid;

pub(super) struct WebDavChapterContext<'a> {
    pub(super) library: &'a Library,
    pub(super) book: &'a Book,
    pub(super) file_urls: &'a [String],
    pub(super) scraper_config: &'a ScraperConfig,
    pub(super) full_scan: bool,
    pub(super) manual_corrected: bool,
    pub(super) json_chapters: Option<Vec<crate::core::books::metadata::AudiobookshelfChapter>>,
    pub(super) chapter_title_template: Option<&'a str>,
    pub(super) ai_chapter_titles: &'a [String],
}

impl LibraryScanner {
    pub(super) async fn process_webdav_chapters(
        &self,
        context: WebDavChapterContext<'_>,
    ) -> Result<bool> {
        let WebDavChapterContext {
            library,
            book,
            file_urls,
            scraper_config,
            full_scan,
            manual_corrected,
            mut json_chapters,
            chapter_title_template,
            ai_chapter_titles,
        } = context;
        let book_id = &book.id;
        let book_title = book.title.as_deref().unwrap_or_default();
        let is_cloud_mode = scraper_config.cloud_mode;
        // Create chapters
        let mut main_counter = 0;
        let mut extra_counter = 0;

        // Fetch book to check for regex rule
        let regex_pattern = if manual_corrected {
            self.book_repo
                .find_by_id(book_id)
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
                    warn!(
                        "metadata.json chapter count ({}) does not match file count ({}) for WebDAV book {}. Ignoring JSON chapters.",
                        chapters.len(),
                        file_urls.len(),
                        book_title
                    );
                }
                false
            }
        } else {
            false
        };
        if use_json_chapters {
            let file_stems: Vec<String> = file_urls
                .iter()
                .map(|file_url| {
                    let decoded_file_url = self.decode_url_path(file_url);
                    let filename = decoded_file_url.split('/').next_back().unwrap_or_default();
                    std::path::Path::new(filename)
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or(filename)
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
                    info!(
                        "Aligned metadata.json chapters to WebDAV files by title for book: {}",
                        book_title
                    );
                }
                json_chapters = Some(aligned);
            }
        }
        let preserve_raw_chapter_titles =
            chapter_title_template_preserves_raw(chapter_title_template);

        for (index, file_url) in file_urls.iter().enumerate() {
            // Decode filename for title
            let decoded_file_url = self.decode_url_path(file_url);
            let filename = decoded_file_url
                .split('/')
                .next_back()
                .unwrap_or("chapter")
                .to_string();
            let ai_chapter_title = ai_chapter_titles
                .get(index)
                .map(|title| title.trim())
                .filter(|title| !title.is_empty());

            // Regex extraction
            let mut regex_idx = None;
            let mut regex_title = None;

            if let Some(re) = &chapter_regex
                && let Some(caps) = re.captures(&filename)
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

            // Check if chapter exists to avoid duplicates
            let mut ch_hasher = Sha256::new();
            ch_hasher.update(file_url.as_bytes());
            let ch_hash = format!("{:x}", ch_hasher.finalize());

            // Read standard containers through the core audio metadata service.
            // In cloud mode we avoid probing WebDAV audio files and rely solely on scraped/sidecar metadata
            let sidecar_chapter = if use_json_chapters {
                json_chapters
                    .as_ref()
                    .and_then(|chapters| chapters.get(index))
            } else {
                None
            };
            let local_title = sidecar_chapter.map(|chapter| chapter.title.as_str());
            let json_duration = sidecar_chapter
                .map(|chapter| ((chapter.end - chapter.start).round() as i32).max(0))
                .filter(|duration| *duration > 0);
            let needs_audio_title = scraper_config.metadata_rank("audio_metadata")
                < scraper_config.metadata_rank(
                    scraper_config
                        .select_title(&filename, local_title, None, None)
                        .1,
                );
            let (audio_title, audio_duration) =
                if !is_cloud_mode && (json_duration.is_none() || needs_audio_title) {
                    let (_, title, _, _, _, duration) = self
                        .extract_webdav_metadata(
                            library,
                            file_url,
                            None,
                            scraper_config.extract_audio_cover,
                        )
                        .await;
                    (title, duration)
                } else {
                    (String::new(), 0)
                };
            let meta_duration = json_duration.unwrap_or(audio_duration);

            // Explicit chapter rules override the configured metadata priority.
            let (raw_title, should_clean_title) = if let Some(rt) = regex_title {
                (rt, true)
            } else {
                let (title, source) =
                    scraper_config.select_title(&filename, local_title, Some(&audio_title), None);
                (title.to_owned(), source != "local_metadata")
            };

            let (final_title, is_extra) = super::super::shared::resolve_chapter_title(
                self.text_cleaner.as_ref(),
                &raw_title,
                book.title.as_deref(),
                ai_chapter_title,
                should_clean_title,
                preserve_raw_chapter_titles,
                scraper_config.extract_extra_chapters,
            );

            let counter_idx = if is_extra {
                extra_counter += 1;
                extra_counter
            } else {
                main_counter += 1;
                main_counter
            };

            let chapter_idx = regex_idx.unwrap_or(counter_idx);
            let final_title = apply_chapter_title_template(
                chapter_title_template,
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
                start_time: None,
                end_time: None,
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

        // Only a full scan has enough information to prove a remote chapter was deleted.
        if full_scan && let Ok(existing_chapters) = self.chapter_repo.find_by_book(book_id).await {
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

        Ok(chapters_changed)
    }
}
