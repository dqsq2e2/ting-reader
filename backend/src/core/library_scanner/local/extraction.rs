use super::super::{LibraryScanner, MetadataSource, STANDARD_EXTENSIONS};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::core::books::metadata::BookMetadataFields as ScannedMetadata;
use crate::core::library_scanner::shared::directory_book_name;

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
        let directory_title = self.text_cleaner.clean_book_directory_title(base_title);
        final_meta.title = Some(directory_title.clone());
        let parent_name = dir
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let name = directory_book_name(base_title, parent_name);
        final_meta.author = name.author.map(str::to_owned);
        final_meta.narrator = name.narrator.map(str::to_owned);

        debug!(
            "Processing dir: {:?}, scraper_config: {:?}",
            dir, scraper_config
        );

        // Read lower-priority sources first, using the same config policy as WebDAV.
        for source_type in scraper_config.metadata_sources().rev() {
            match source_type {
                "filename" => {
                    final_meta.title = Some(directory_title.clone());
                    final_source = MetadataSource::Fallback;
                }
                "local_metadata" => {
                    if let Some(path) = self.find_cover_image(dir) {
                        final_meta.cover_url = Some(path);
                    }
                    for metadata in [self.extract_from_nfo(dir), self.extract_from_json(dir)]
                        .into_iter()
                        .flatten()
                    {
                        if metadata
                            .title
                            .as_ref()
                            .is_some_and(|title| !title.trim().is_empty())
                        {
                            final_source = MetadataSource::Nfo;
                        }
                        final_meta.merge(metadata);
                    }
                }
                "audio_metadata" => {
                    if let Some(meta) = self
                        .extract_from_audio(dir, files, scraper_config.extract_audio_cover)
                        .await
                    {
                        let mut metadata = meta;
                        if metadata.title.as_ref().is_some_and(|title| {
                            title.trim().is_empty() || title.to_lowercase().starts_with("track")
                        }) {
                            metadata.title = None;
                        }
                        if metadata.title.is_some() {
                            final_source = MetadataSource::FileMetadata;
                        }
                        final_meta.merge(metadata);
                    }
                }
                "scraper" if scraper_config.has_scraper_sources() => {
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
                            if meta
                                .title
                                .as_ref()
                                .is_some_and(|title| !title.trim().is_empty())
                            {
                                final_source = MetadataSource::Scraper;
                            }
                            final_meta.merge(meta);
                        }
                    }
                }
                _ => {}
            }
        }

        // 2. Post-processing: Cover Image
        // If no cover URL yet, try finding local file (cover.jpg)
        if final_meta.cover_url.is_none()
            && let Some(path) = self.find_cover_image(dir)
        {
            final_meta.cover_url = Some(path);
        }

        // 3. Fallback Cover Extraction (if still no cover, extract from ID3 and save)
        // This runs only if cover is still missing, regardless of priority,
        // because if "audio_metadata" was high priority, it would have set cover_url from plugin/id3.
        if scraper_config.extract_audio_cover && final_meta.cover_url.is_none() && !files.is_empty()
        {
            let first_file = &files[0];
            // We've already tried extracting cover in extract_from_audio above for both standard and non-standard.
            // This is a final fallback just in case the file wasn't picked up by the priority system.
            if let Some(path) = self.extract_and_save_cover(first_file, dir).await {
                final_meta.cover_url = Some(path);
            } else {
                // Try extracting cover from non-standard files (like .xm) via plugin
                if let Some(meta) = self.extract_from_audio(dir, files, true).await
                    && meta.cover_url.is_some()
                {
                    final_meta.cover_url = meta.cover_url;
                }
            }
        }

        // 4. Validate local cover paths to ensure they exist
        if let Some(ref url) = final_meta.cover_url
            && !url.starts_with("http")
            && !url.starts_with("//")
        {
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

        (final_meta, final_source)
    }

    fn extract_from_nfo(&self, dir: &Path) -> Option<ScannedMetadata> {
        self.nfo_manager
            .read_book_nfo(&dir.join("book.nfo"))
            .ok()
            .map(Into::into)
    }

    fn extract_from_json(&self, dir: &Path) -> Option<ScannedMetadata> {
        match crate::core::books::metadata::read_metadata_json(dir) {
            Ok(metadata) => metadata.map(Into::into),
            Err(error) => {
                warn!(path = %dir.display(), %error, "Failed to read metadata.json");
                None
            }
        }
    }

    async fn extract_from_audio(
        &self,
        dir: &Path,
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
            let mut plugin_handled = false;

            if let Ok(Some(extracted)) = self
                .plugin_manager
                .extract_local_format_metadata(file_path, extract_cover)
                .await
                && let Ok(result) = extracted.into_scanner_json(Some(dir))
            {
                if index == 0 {
                    tracing::debug!(
                        "Using format plugin {} to process the first {} file",
                        "format_handler",
                        ext
                    );
                } else {
                    tracing::debug!(
                        "Using format plugin {} to process file #{} ({}) for supplemental metadata",
                        "format_handler",
                        index + 1,
                        ext
                    );
                }

                // 只在第一个文件或缺失时提取基本元数据
                if (index == 0 || m.title.is_none())
                    && let Some(t) = result.get("album").and_then(|v| v.as_str())
                    && !t.trim().is_empty()
                {
                    m.title = Some(t.to_string());
                    found = true;
                }
                if index == 0 || m.author.is_none() {
                    if let Some(aa) = result.get("album_artist").and_then(|v| v.as_str())
                        && !aa.trim().is_empty()
                    {
                        m.author = Some(aa.to_string());
                    }
                    if let Some(a) = result.get("artist").and_then(|v| v.as_str())
                        && !a.trim().is_empty()
                    {
                        if m.author.is_none() {
                            m.author = Some(a.to_string());
                        } else if m.author.as_deref() != Some(a) && m.narrator.is_none() {
                            m.narrator = Some(a.to_string());
                        }
                    }
                }
                if (index == 0 || m.narrator.is_none())
                    && let Some(n) = result.get("narrator").and_then(|v| v.as_str())
                    && !n.trim().is_empty()
                {
                    m.narrator = Some(n.to_string());
                }

                // 封面：如果还没有找到，继续尝试
                if extract_cover
                    && m.cover_url.is_none()
                    && let Some(c) = result.get("cover_url").and_then(|v| v.as_str())
                    && !c.trim().is_empty()
                {
                    m.cover_url = Some(c.to_string());
                    found = true;
                    if index > 0 {
                        tracing::info!("Found cover in file #{}", index + 1);
                    }
                }

                if (index == 0 || m.description.is_none())
                    && let Some(d) = result.get("description").and_then(|v| v.as_str())
                    && !d.trim().is_empty()
                {
                    m.description = Some(d.to_string());
                }
                if (index == 0 || m.genre.is_none())
                    && let Some(g) = result.get("genre").and_then(|v| v.as_str())
                    && !g.trim().is_empty()
                {
                    m.genre = Some(g.to_string());
                }

                plugin_handled = true;
            }

            // Standard containers use the same core reader as remote sources.
            if !plugin_handled
                && is_standard
                && let Ok(meta) = crate::core::audio::AudioService::read_file_metadata(
                    &crate::core::audio::metadata::AudioInput::local(file_path),
                    extract_cover.then_some(dir),
                )
                .await
            {
                if index == 0 {
                    tracing::debug!(
                        "Using core audio metadata to process the first {} file",
                        ext
                    );
                    tracing::debug!(
                        embedded_chapters = meta.chapters.len(),
                        codec = ?meta.codec,
                        sample_rate = ?meta.sample_rate,
                        channels = ?meta.channels,
                        bit_depth = ?meta.bit_depth,
                        bitrate_kbps = ?meta.bitrate_kbps,
                        "Audio stream details"
                    );
                } else {
                    tracing::debug!(
                        "Using core audio metadata to process file #{} ({}) for supplemental metadata",
                        index + 1,
                        ext
                    );
                }

                if (index == 0 || m.title.is_none())
                    && let Some(t) = meta.album
                    && !t.trim().is_empty()
                {
                    m.title = Some(t);
                    found = true;
                }
                if index == 0 || m.author.is_none() {
                    if let Some(aa) = meta.album_artist
                        && !aa.trim().is_empty()
                    {
                        m.author = Some(aa);
                    }
                    if let Some(a) = meta.artist
                        && !a.trim().is_empty()
                    {
                        if m.author.is_none() {
                            m.author = Some(a.clone());
                        } else if m.author.as_ref() != Some(&a) && m.narrator.is_none() {
                            m.narrator = Some(a);
                        }
                    }
                }
                if (index == 0 || m.narrator.is_none())
                    && let Some(c) = meta.composer
                    && !c.trim().is_empty()
                    && m.narrator.is_none()
                {
                    m.narrator = Some(c);
                }
                if (index == 0 || m.genre.is_none())
                    && let Some(g) = meta.genre
                    && !g.trim().is_empty()
                {
                    m.genre = Some(g);
                }

                if (index == 0 || m.subtitle.is_none())
                    && let Some(subtitle) = meta.subtitle
                    && !subtitle.trim().is_empty()
                {
                    m.subtitle = Some(subtitle);
                }
                if (index == 0 || m.publisher.is_none())
                    && let Some(publisher) = meta.publisher
                    && !publisher.trim().is_empty()
                {
                    m.publisher = Some(publisher);
                }
                if (index == 0 || m.published_date.is_none())
                    && let Some(date) = meta.published_date
                    && !date.trim().is_empty()
                {
                    if m.published_year.is_none() {
                        m.published_year = date
                            .get(0..4)
                            .filter(|year| year.chars().all(|ch| ch.is_ascii_digit()))
                            .map(str::to_string);
                    }
                    m.published_date = Some(date);
                }
                if (index == 0 || m.language.is_none())
                    && let Some(language) = meta.language
                    && !language.trim().is_empty()
                {
                    m.language = Some(language);
                }
                if (index == 0 || m.asin.is_none())
                    && let Some(asin) = meta.asin
                    && !asin.trim().is_empty()
                {
                    m.asin = Some(asin);
                }
                if (index == 0 || m.isbn.is_none())
                    && let Some(isbn) = meta.isbn
                    && !isbn.trim().is_empty()
                {
                    m.isbn = Some(isbn);
                }
                if meta.explicit {
                    m.explicit = true;
                }

                if (index == 0 || m.description.is_none())
                    && let Some(description) = meta.description
                {
                    m.description = Some(description);
                }
                if extract_cover
                    && m.cover_url.is_none()
                    && let Some(path) = meta.cover_url
                {
                    m.cover_url = Some(path);
                    found = true;
                    if index > 0 {
                        tracing::info!("Found cover in file #{}", index + 1);
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

        found |= m.author.is_some()
            || m.narrator.is_some()
            || m.description.is_some()
            || m.genre.is_some();
        info!(
            "extract_from_audio: returning found={}, meta={:?}",
            found, m
        );
        if found { Some(m) } else { None }
    }
    async fn extract_from_scraper(
        &self,
        title: &str,
        _author: &Option<String>,
        scraper_config: &crate::db::models::ScraperConfig,
        context: serde_json::Value,
    ) -> Option<ScannedMetadata> {
        if scraper_config.has_scraper_sources()
            && let Some(scraper) = &self.scraper_service
        {
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
                if let Some(g) = detail.genre
                    && !g.trim().is_empty()
                {
                    m.genre = Some(g);
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

    async fn extract_and_save_cover(&self, audio_path: &Path, book_dir: &Path) -> Option<String> {
        crate::core::audio::AudioService::read_file_metadata(
            &crate::core::audio::metadata::AudioInput::local(audio_path),
            Some(book_dir),
        )
        .await
        .ok()
        .and_then(|metadata| metadata.cover_url)
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
