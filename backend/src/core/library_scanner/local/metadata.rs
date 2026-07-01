use super::super::{LibraryScanner, MetadataSource, STANDARD_EXTENSIONS};
use crate::plugin::manager::FormatMethod;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Debug, Default, Clone, serde::Serialize)]
pub(crate) struct ScannedMetadata {
    pub(crate) title: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) narrator: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) tags: Option<String>,
    pub(crate) genre: Option<String>,
    pub(crate) cover_url: Option<String>,
    pub(crate) subtitle: Option<String>,
    pub(crate) published_year: Option<String>,
    pub(crate) published_date: Option<String>,
    pub(crate) publisher: Option<String>,
    pub(crate) isbn: Option<String>,
    pub(crate) asin: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) explicit: bool,
    pub(crate) abridged: bool,
    pub(crate) json_tags: Vec<String>,
    pub(crate) json_series: Vec<String>,
    pub(crate) json_chapters: Option<Vec<crate::core::metadata_writer::AudiobookshelfChapter>>,
    pub(crate) chapter_title_template: Option<String>,
    pub(crate) chapter_titles: Vec<String>,
}

impl ScannedMetadata {
    fn merge(&mut self, other: ScannedMetadata) {
        if let Some(t) = other.title {
            if !t.trim().is_empty() {
                self.title = Some(t);
            }
        }
        if other.author.is_some() {
            self.author = other.author;
        }
        if other.narrator.is_some() {
            self.narrator = other.narrator;
        }
        if other.description.is_some() {
            self.description = other.description;
        }
        if other.tags.is_some() {
            self.tags = other.tags;
        }
        if other.genre.is_some() {
            self.genre = other.genre;
        }
        if let Some(c) = other.cover_url {
            if !c.trim().is_empty() {
                self.cover_url = Some(c);
            }
        }
        if other.subtitle.is_some() {
            self.subtitle = other.subtitle;
        }
        if other.published_year.is_some() {
            self.published_year = other.published_year;
        }
        if other.published_date.is_some() {
            self.published_date = other.published_date;
        }
        if other.publisher.is_some() {
            self.publisher = other.publisher;
        }
        if other.isbn.is_some() {
            self.isbn = other.isbn;
        }
        if other.asin.is_some() {
            self.asin = other.asin;
        }
        if other.language.is_some() {
            self.language = other.language;
        }
        if other.explicit {
            self.explicit = true;
        }
        if other.abridged {
            self.abridged = true;
        }
        if !other.json_tags.is_empty() {
            self.json_tags = other.json_tags;
        }
        if !other.json_series.is_empty() {
            self.json_series = other.json_series;
        }
        if other.json_chapters.is_some() {
            self.json_chapters = other.json_chapters;
        }
        if other.chapter_title_template.is_some() {
            self.chapter_title_template = other.chapter_title_template;
        }
        if !other.chapter_titles.is_empty() {
            self.chapter_titles = other.chapter_titles;
        }
    }
}

impl LibraryScanner {
    pub(super) async fn extract_final_metadata(
        &self,
        dir: &Path,
        files: &[PathBuf],
        scraper_config: &crate::db::models::ScraperConfig,
        fallback_title_override: Option<&str>,
    ) -> (ScannedMetadata, MetadataSource) {
        let mut final_meta = ScannedMetadata::default();
        let mut final_source = MetadataSource::Fallback;

        // 0. Base: Directory Name
        let dir_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown Book");
        let base_title = fallback_title_override
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(dir_name);
        let (cleaned_title, _) = self.text_cleaner.clean_chapter_title(base_title, None);
        final_meta.title = Some(cleaned_title);

        // Fallback Author from "Author - Title" pattern
        if base_title.contains(" - ") {
            let parts: Vec<&str> = base_title.split(" - ").collect();
            if parts.len() >= 2 {
                final_meta.author = Some(parts[0].trim().to_string());
                // Also update title if we are assuming Author - Title pattern
                if final_meta.title.is_some() && final_meta.title.as_deref() == Some(base_title) {
                    let (cleaned_title_part, _) =
                        self.text_cleaner.clean_chapter_title(parts[1], None);
                    final_meta.title = Some(cleaned_title_part);
                }
            }
        }

        // 1. Check if we should force filename as title
        debug!(
            "Processing dir: {:?}, scraper_config: {:?}",
            dir, scraper_config
        );

        // 2. Iterate Priority List
        let default_priority = vec![
            "scraper".to_string(),
            "audio_metadata".to_string(),
            "local_metadata".to_string(),
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
                }
                "audio_metadata" => {
                    if let Some(meta) = self
                        .extract_from_audio(dir, files, scraper_config.extract_audio_cover)
                        .await
                    {
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
                                if lower_title.starts_with("track") || lower_title.trim().is_empty()
                                {
                                    m.title = None; // Ignore bad embedded titles
                                }
                            }

                            final_meta.merge(m);
                            if final_meta.title.is_some() {
                                final_source = MetadataSource::FileMetadata;
                            }
                        }
                    }
                }
                "scraper" => {
                    if let Some(ref title) = final_meta.title {
                        let context = serde_json::json!({
                            "library_type": "local",
                            "directory": dir.to_string_lossy(),
                            "directory_name": base_title,
                            "chapters": build_chapter_title_candidates(files),
                            "current_metadata": final_meta,
                        });
                        if let Some(meta) = self
                            .extract_from_scraper(
                                title,
                                &final_meta.author,
                                scraper_config,
                                context,
                            )
                            .await
                        {
                            if scraper_config.use_filename_as_title {
                                let mut m = meta.clone();
                                m.title = None;
                                final_meta.merge(m);
                            } else {
                                final_meta.merge(meta);
                            }
                        }
                    }
                }
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
        if scraper_config.extract_audio_cover && final_meta.cover_url.is_none() && !files.is_empty()
        {
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
                title: if meta.title.is_empty() {
                    None
                } else {
                    Some(meta.title)
                },
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
                if !meta.authors.is_empty() {
                    m.author = Some(meta.authors[0].clone());
                }
                if !meta.narrators.is_empty() {
                    m.narrator = Some(meta.narrators[0].clone());
                }
                m.description = meta.description;
                if !meta.genres.is_empty() {
                    m.genre = Some(meta.genres.join(","));
                }
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
                if !meta.chapters.is_empty() {
                    m.json_chapters = Some(meta.chapters);
                }
                Some(m)
            }
            Ok(None) => None,
            Err(e) => {
                warn!("Failed to read metadata.json in {:?}: {}", dir, e);
                None
            }
        }
    }

    async fn extract_from_audio(
        &self,
        _dir: &Path,
        files: &[PathBuf],
        extract_cover: bool,
    ) -> Option<ScannedMetadata> {
        if files.is_empty() {
            return None;
        }

        let mut m = ScannedMetadata::default();
        let mut found = false;

        // 策略：优先使用格式插件（支持更多格式，对部分文件更友好）
        // 如果第一个文件没有封面，尝试其他文件

        // 尝试多个文件，直到找到完整的元数据（包括封面）
        for (index, file_path) in files.iter().enumerate() {
            let ext = file_path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let is_standard = STANDARD_EXTENSIONS.contains(&ext.as_str());

            // 1. 优先尝试格式插件
            let plugins = self
                .plugin_manager
                .find_plugins_by_capability_kind("format_handler")
                .await;
            let mut plugin_handled = false;

            for plugin in plugins {
                let supports_ext = plugin
                    .supported_extensions
                    .as_ref()
                    .map(|exts| exts.iter().any(|e| e.eq_ignore_ascii_case(&ext)))
                    .unwrap_or(false);
                if !supports_ext {
                    continue;
                }

                let params = serde_json::json!({ "file_path": file_path.to_string_lossy(), "extract_cover": extract_cover });
                if let Ok(result) = self
                    .plugin_manager
                    .call_format(&plugin.id, FormatMethod::ExtractMetadata, params)
                    .await
                {
                    if index == 0 {
                        tracing::debug!(
                            "Using format plugin {} to process the first {} file",
                            plugin.name,
                            ext
                        );
                    } else {
                        tracing::debug!(
                            "Using format plugin {} to process file #{} ({}) for supplemental metadata",
                            plugin.name,
                            index + 1,
                            ext
                        );
                    }

                    // 只在第一个文件或缺失时提取基本元数据
                    if index == 0 || m.title.is_none() {
                        if let Some(t) = result.get("album").and_then(|v| v.as_str()) {
                            if !t.trim().is_empty() {
                                m.title = Some(t.to_string());
                                found = true;
                            }
                        }
                    }
                    if index == 0 || m.author.is_none() {
                        if let Some(aa) = result.get("album_artist").and_then(|v| v.as_str()) {
                            if !aa.trim().is_empty() {
                                m.author = Some(aa.to_string());
                            }
                        }
                        if let Some(a) = result.get("artist").and_then(|v| v.as_str()) {
                            if !a.trim().is_empty() {
                                if m.author.is_none() {
                                    m.author = Some(a.to_string());
                                } else if m.author.as_ref().map(|s| s.as_str()) != Some(a)
                                    && m.narrator.is_none()
                                {
                                    m.narrator = Some(a.to_string());
                                }
                            }
                        }
                    }
                    if index == 0 || m.narrator.is_none() {
                        if let Some(n) = result.get("narrator").and_then(|v| v.as_str()) {
                            if !n.trim().is_empty() {
                                m.narrator = Some(n.to_string());
                            }
                        }
                    }

                    // 封面：如果还没有找到，继续尝试
                    if extract_cover && m.cover_url.is_none() {
                        if let Some(c) = result.get("cover_url").and_then(|v| v.as_str()) {
                            if !c.trim().is_empty() {
                                m.cover_url = Some(c.to_string());
                                found = true;
                                if index > 0 {
                                    tracing::info!("Found cover in file #{}", index + 1);
                                }
                            }
                        }
                    }

                    if index == 0 || m.description.is_none() {
                        if let Some(d) = result.get("description").and_then(|v| v.as_str()) {
                            if !d.trim().is_empty() {
                                m.description = Some(d.to_string());
                            }
                        }
                    }
                    if index == 0 || m.genre.is_none() {
                        if let Some(g) = result.get("genre").and_then(|v| v.as_str()) {
                            if !g.trim().is_empty() {
                                m.genre = Some(g.to_string());
                            }
                        }
                    }

                    plugin_handled = true;
                    break;
                }
            }

            // 2. 如果插件没有处理，且是标准格式，尝试 Symphonia（仅用于完整文件）
            if !plugin_handled && is_standard {
                if let Ok(meta) = self.audio_streamer.read_metadata(file_path) {
                    if index == 0 {
                        tracing::debug!("Using Symphonia to process the first {} file", ext);
                    } else {
                        tracing::debug!(
                            "Using Symphonia to process file #{} ({}) for supplemental metadata",
                            index + 1,
                            ext
                        );
                    }

                    if index == 0 || m.title.is_none() {
                        if let Some(t) = meta.album {
                            if !t.trim().is_empty() {
                                m.title = Some(t);
                                found = true;
                            }
                        }
                    }
                    if index == 0 || m.author.is_none() {
                        if let Some(aa) = meta.album_artist {
                            if !aa.trim().is_empty() {
                                m.author = Some(aa);
                            }
                        }
                        if let Some(a) = meta.artist {
                            if !a.trim().is_empty() {
                                if m.author.is_none() {
                                    m.author = Some(a.clone());
                                } else if m.author.as_ref() != Some(&a) && m.narrator.is_none() {
                                    m.narrator = Some(a);
                                }
                            }
                        }
                    }
                    if index == 0 || m.narrator.is_none() {
                        if let Some(c) = meta.composer {
                            if !c.trim().is_empty() && m.narrator.is_none() {
                                m.narrator = Some(c);
                            }
                        }
                    }
                    if index == 0 || m.genre.is_none() {
                        if let Some(g) = meta.genre {
                            if !g.trim().is_empty() {
                                m.genre = Some(g);
                            }
                        }
                    }

                    // Symphonia 不提取封面，使用 extract_and_save_cover
                    if extract_cover && m.cover_url.is_none() {
                        if let Some(path) = self.extract_and_save_cover(file_path, _dir) {
                            m.cover_url = Some(path);
                            found = true;
                            if index > 0 {
                                tracing::info!("Found cover in file #{}", index + 1);
                            }
                        }
                    }
                }
            }

            // 如果已经找到了所有需要的元数据，就停止
            // 基本元数据：title 或 author
            // 如果需要封面：cover_url
            let has_basic_metadata = m.title.is_some() || m.author.is_some();
            let has_cover_if_needed = !extract_cover || m.cover_url.is_some();

            if has_basic_metadata && has_cover_if_needed {
                tracing::debug!("Complete metadata found; stopping file attempts");
                break;
            }

            // 限制尝试的文件数量，避免扫描太多文件（最多 3 个）
            if index >= 2 {
                tracing::debug!("Tried 3 files; stopping");
                break;
            }
        }

        info!(
            "extract_from_audio: returning found={}, meta={:?}",
            found, m
        );
        if found {
            Some(m)
        } else {
            None
        }
    }
    async fn extract_from_scraper(
        &self,
        title: &str,
        _author: &Option<String>,
        scraper_config: &crate::db::models::ScraperConfig,
        context: serde_json::Value,
    ) -> Option<ScannedMetadata> {
        if let Some(scraper) = &self.scraper_service {
            // Basic scrape check
            if let Ok(detail) = scraper
                .scrape_book_metadata_with_context(title, scraper_config, Some(context))
                .await
            {
                let mut m = ScannedMetadata::default();
                if !detail.intro.is_empty() {
                    m.description = Some(detail.intro);
                }
                if !detail.tags.is_empty() {
                    m.tags = Some(detail.tags.join(","));
                }
                if let Some(g) = detail.genre {
                    if !g.trim().is_empty() {
                        m.genre = Some(g);
                    }
                }
                m.cover_url = detail.cover_url;
                m.narrator = detail.narrator;
                if !detail.author.is_empty() {
                    m.author = Some(detail.author);
                }
                m.subtitle = detail.subtitle;
                m.published_year = detail.published_year;
                m.published_date = detail.published_date;
                m.publisher = detail.publisher;
                m.isbn = detail.isbn;
                m.asin = detail.asin;
                m.language = detail.language;
                if detail.explicit {
                    m.explicit = true;
                }
                if detail.abridged {
                    m.abridged = true;
                }
                m.chapter_title_template = detail.chapter_title_template;
                m.chapter_titles = detail.chapter_titles;
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
                info!(
                    "Cover file already exists at {:?}, skipping extraction",
                    cover_path
                );
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
            let picture = tag
                .pictures()
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
}

fn build_chapter_title_candidates(files: &[PathBuf]) -> Vec<serde_json::Value> {
    files
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            let title = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or(&filename)
                .to_string();

            serde_json::json!({
                "index": index + 1,
                "filename": filename,
                "title": title,
                "path": path.to_string_lossy(),
            })
        })
        .collect()
}
