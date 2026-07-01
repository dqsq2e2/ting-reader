use super::{LibraryScanner, ScanMode, ScanResult, ScanStatus};
use crate::core::error::{Result, TingError};
use crate::db::models::{Book, Chapter, Library};
use crate::db::repository::Repository;
use chrono::{DateTime, Utc};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tracing::{info, warn};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Default)]
struct RssFeed {
    title: Option<String>,
    author: Option<String>,
    narrator: Option<String>,
    description: Option<String>,
    image: Option<String>,
    categories: Vec<String>,
    episodes: Vec<RssEpisode>,
}

#[derive(Debug, Default, Clone)]
struct RssEpisode {
    title: Option<String>,
    description: Option<String>,
    author: Option<String>,
    guid: Option<String>,
    enclosure_url: Option<String>,
    enclosure_type: Option<String>,
    enclosure_size: Option<u64>,
    pub_date: Option<String>,
    published_at: Option<DateTime<Utc>>,
    episode_number: Option<i32>,
    duration: Option<i32>,
}

impl LibraryScanner {
    pub async fn scan_rss_library(
        &self,
        library: &Library,
        task_id: Option<&str>,
        mode: ScanMode,
    ) -> Result<ScanResult> {
        let mut result = ScanResult {
            start_time: Some(std::time::Instant::now()),
            ..Default::default()
        };

        self.update_progress_key(
            task_id,
            "scan.rss.fetching",
            serde_json::json!({ "url": library.url }),
        )
        .await;

        let response = self
            .http_client
            .get(&library.url)
            .header(
                "Accept",
                "application/rss+xml, application/xml, text/xml, */*",
            )
            .header(
                "User-Agent",
                "TingReader/1.0 (+https://github.com/ting-reader)",
            )
            .send()
            .await
            .map_err(|e| TingError::NetworkError(format!("Failed to fetch RSS feed: {}", e)))?;

        if !response.status().is_success() {
            return Err(TingError::NetworkError(format!(
                "RSS feed returned status {}",
                response.status()
            )));
        }

        let feed_bytes = response
            .bytes()
            .await
            .map_err(|e| TingError::NetworkError(format!("Failed to read RSS feed: {}", e)))?;
        let feed_xml = String::from_utf8_lossy(&feed_bytes);
        let base_url =
            Url::parse(&library.url).map_err(|e| TingError::ValidationError(e.to_string()))?;
        let mut feed = parse_rss_feed(&feed_xml, &base_url)?;

        feed.episodes.retain(|episode| {
            episode
                .enclosure_url
                .as_deref()
                .is_some_and(|url| url.starts_with("http://") || url.starts_with("https://"))
        });

        if feed.episodes.is_empty() {
            return Err(TingError::ValidationError(
                "RSS feed contains no playable audio enclosures".to_string(),
            ));
        }

        for episode in &mut feed.episodes {
            if episode.episode_number.is_none() {
                episode.episode_number = episode
                    .title
                    .as_deref()
                    .and_then(parse_title_episode_number);
            }
        }

        if feed.author.is_none() {
            feed.author = feed
                .episodes
                .iter()
                .find_map(|episode| episode.author.clone());
        }
        if feed.narrator.is_none() {
            feed.narrator = feed.author.clone();
        }
        if feed.author.is_none() {
            feed.author = feed.narrator.clone();
        }

        let has_episode_numbers = feed
            .episodes
            .iter()
            .any(|episode| episode.episode_number.is_some());
        if has_episode_numbers {
            feed.episodes.sort_by(|a, b| {
                let a_idx = a.episode_number.unwrap_or(i32::MAX);
                let b_idx = b.episode_number.unwrap_or(i32::MAX);
                a_idx.cmp(&b_idx)
            });
        } else if feed
            .episodes
            .iter()
            .any(|episode| episode.published_at.is_some())
        {
            feed.episodes.sort_by(|a, b| {
                let a_ts = a
                    .published_at
                    .map(|date| date.timestamp())
                    .unwrap_or_default();
                let b_ts = b
                    .published_at
                    .map(|date| date.timestamp())
                    .unwrap_or_default();
                a_ts.cmp(&b_ts)
            });
        }

        self.update_progress_key(
            task_id,
            "scan.rss.fetched",
            serde_json::json!({ "count": feed.episodes.len() }),
        )
        .await;

        let book_hash = hash_string(&format!("rss:{}", library.id));
        let book_id = self
            .upsert_rss_book(library, &feed, &book_hash, &mut result)
            .await?;

        let chapters_changed = self
            .upsert_rss_chapters(&book_id, &feed.episodes, mode)
            .await?;

        if chapters_changed && result.books_updated == 0 && result.books_created == 0 {
            result.books_skipped = result.books_skipped.saturating_sub(1);
            result.books_updated = 1;
        }

        result.total_books = 1;
        result.end_time = Some(std::time::Instant::now());
        info!(
            target: "audit::scan",
            message_key = "scan.rss.completed",
            message_params = %serde_json::json!({
                "library_id": library.id,
                "episodes": feed.episodes.len(),
            }),
            library_id = %library.id,
            episodes = feed.episodes.len(),
            "RSS library scan completed"
        );

        Ok(result)
    }

    async fn upsert_rss_book(
        &self,
        library: &Library,
        feed: &RssFeed,
        book_hash: &str,
        result: &mut ScanResult,
    ) -> Result<String> {
        let existing_books = self.book_repo.find_by_library(&library.id).await?;
        let existing = existing_books
            .iter()
            .find(|book| book.hash == book_hash)
            .cloned()
            .or_else(|| {
                existing_books
                    .iter()
                    .find(|book| book.path == library.url)
                    .cloned()
            })
            .or_else(|| {
                if existing_books.len() == 1 {
                    existing_books.first().cloned()
                } else {
                    None
                }
            });

        let book_id = existing
            .as_ref()
            .map(|book| book.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let existing_cover_url = existing.as_ref().and_then(|book| book.cover_url.clone());
        let existing_theme_color = existing.as_ref().and_then(|book| book.theme_color.clone());
        let categories = normalized_categories(&feed.categories);
        let mut book = Book {
            id: book_id.clone(),
            library_id: library.id.clone(),
            title: feed.title.clone().or_else(|| Some(library.name.clone())),
            author: feed.author.clone().or_else(|| feed.narrator.clone()),
            narrator: feed.narrator.clone().or_else(|| feed.author.clone()),
            cover_url: feed.image.clone(),
            theme_color: None,
            description: feed.description.clone(),
            skip_intro: 0,
            skip_outro: 0,
            path: library.url.clone(),
            hash: book_hash.to_string(),
            tags: categories.clone(),
            genre: categories,
            year: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            manual_corrected: existing
                .as_ref()
                .map(|book| book.manual_corrected)
                .unwrap_or(0),
            match_pattern: existing
                .as_ref()
                .and_then(|book| book.match_pattern.clone()),
            chapter_regex: existing
                .as_ref()
                .and_then(|book| book.chapter_regex.clone()),
        };

        let status = if let Some(existing) = existing {
            book.created_at = existing.created_at;
            if existing.manual_corrected != 0 {
                book.title = existing.title;
                book.author = existing.author;
                book.narrator = existing.narrator;
                book.cover_url = existing.cover_url;
                book.theme_color = existing.theme_color;
                book.description = existing.description;
                book.tags = existing.tags;
                book.genre = existing.genre;
                book.year = existing.year;
                book.skip_intro = existing.skip_intro;
                book.skip_outro = existing.skip_outro;
                book.manual_corrected = existing.manual_corrected;
                self.book_repo.update(&book).await?;
                ScanStatus::Skipped
            } else {
                fill_rss_theme_color(
                    &mut book,
                    existing_cover_url.as_deref(),
                    existing_theme_color,
                    &self.http_client,
                )
                .await;
                self.book_repo.update(&book).await?;
                ScanStatus::Updated
            }
        } else {
            fill_rss_theme_color(&mut book, None, None, &self.http_client).await;
            self.book_repo.create(&book).await?;
            ScanStatus::Created
        };

        match status {
            ScanStatus::Created => result.books_created += 1,
            ScanStatus::Updated => result.books_updated += 1,
            ScanStatus::Skipped => result.books_skipped += 1,
        }

        Ok(book_id)
    }

    async fn upsert_rss_chapters(
        &self,
        book_id: &str,
        episodes: &[RssEpisode],
        mode: ScanMode,
    ) -> Result<bool> {
        let existing_chapters = self.chapter_repo.find_by_book(book_id).await?;
        let mut existing_by_hash: HashMap<String, Chapter> = existing_chapters
            .iter()
            .filter_map(|chapter| {
                chapter
                    .hash
                    .as_ref()
                    .map(|hash| (hash.clone(), chapter.clone()))
            })
            .collect();

        let mut processed_ids = HashSet::new();
        let mut changed = false;

        for (index, episode) in episodes.iter().enumerate() {
            let Some(enclosure_url) = episode.enclosure_url.as_ref() else {
                continue;
            };
            let stable_key = episode
                .guid
                .as_ref()
                .filter(|guid| !guid.trim().is_empty())
                .unwrap_or(enclosure_url);
            let chapter_hash = hash_string(&format!("rss:{}:{}", book_id, stable_key));
            let title = episode
                .title
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("Episode {}", index + 1));

            let chapter_index = episode.episode_number.unwrap_or((index + 1) as i32);

            if let Some(mut chapter) = existing_by_hash.remove(&chapter_hash) {
                if chapter.manual_corrected == 0 {
                    chapter.title = Some(title);
                    chapter.chapter_index = Some(chapter_index);
                    chapter.is_extra = 0;
                }
                chapter.path = enclosure_url.clone();
                chapter.duration = episode.duration;
                chapter.book_id = book_id.to_string();
                chapter.hash = Some(chapter_hash);
                self.chapter_repo.update(&chapter).await?;
                processed_ids.insert(chapter.id);
                changed = true;
            } else {
                let chapter = Chapter {
                    id: Uuid::new_v4().to_string(),
                    book_id: book_id.to_string(),
                    title: Some(title),
                    path: enclosure_url.clone(),
                    duration: episode.duration,
                    chapter_index: Some(chapter_index),
                    is_extra: 0,
                    hash: Some(chapter_hash),
                    manual_corrected: 0,
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                self.chapter_repo.create(&chapter).await?;
                processed_ids.insert(chapter.id);
                changed = true;
            }
        }

        if mode.is_full() {
            for chapter in existing_chapters {
                if !processed_ids.contains(&chapter.id) {
                    info!(
                        "Removing RSS chapter no longer present in feed: {}",
                        chapter.id
                    );
                    if let Err(e) = self.chapter_repo.delete(&chapter.id).await {
                        warn!("Failed to delete removed RSS chapter {}: {}", chapter.id, e);
                    } else {
                        changed = true;
                    }
                }
            }
        }

        Ok(changed)
    }
}

async fn fill_rss_theme_color(
    book: &mut Book,
    previous_cover_url: Option<&str>,
    previous_theme_color: Option<String>,
    client: &reqwest::Client,
) {
    let Some(cover_url) = book.cover_url.as_deref() else {
        book.theme_color = None;
        return;
    };

    if previous_cover_url == Some(cover_url) {
        if previous_theme_color.is_some() {
            book.theme_color = previous_theme_color;
            return;
        }
    }

    let cover_path = if cover_url.starts_with("//") {
        format!("https:{}", cover_url)
    } else {
        cover_url.to_string()
    };

    match crate::core::color::calculate_theme_color_with_client(&cover_path, client).await {
        Ok(Some(color)) => {
            book.theme_color = Some(color);
        }
        Ok(None) => {
            book.theme_color = None;
        }
        Err(err) => {
            warn!(
                message_key = "book.theme_color.calculate_failed",
                message_params = %serde_json::json!({
                    "book_id": book.id.as_str(),
                    "cover_url": cover_url,
                    "error": err.to_string(),
                }),
                book_id = %book.id,
                cover_url = %cover_url,
                error = %err,
                "RSS cover theme color analysis failed"
            );
            book.theme_color = None;
        }
    }
}

fn parse_rss_feed(xml: &str, base_url: &Url) -> Result<RssFeed> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut feed = RssFeed::default();
    let mut current_episode: Option<RssEpisode> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = tag_name(event.name().as_ref());
                if tag == "item" || tag == "entry" {
                    current_episode = Some(RssEpisode::default());
                }
                handle_attrs(&event, &tag, base_url, &mut feed, &mut current_episode);
                stack.push(tag);
            }
            Ok(Event::Empty(event)) => {
                let tag = tag_name(event.name().as_ref());
                handle_attrs(&event, &tag, base_url, &mut feed, &mut current_episode);
            }
            Ok(Event::Text(text)) => {
                let value = text
                    .unescape()
                    .map(|cow| cow.to_string())
                    .unwrap_or_else(|_| String::from_utf8_lossy(text.as_ref()).to_string());
                handle_text(&stack, &value, &mut feed, &mut current_episode);
            }
            Ok(Event::CData(text)) => {
                let value = String::from_utf8_lossy(text.as_ref()).to_string();
                handle_text(&stack, &value, &mut feed, &mut current_episode);
            }
            Ok(Event::End(event)) => {
                let tag = tag_name(event.name().as_ref());
                if (tag == "item" || tag == "entry") && current_episode.is_some() {
                    if let Some(episode) = current_episode.take() {
                        feed.episodes.push(episode);
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(TingError::DeserializationError(format!(
                    "Failed to parse RSS XML: {}",
                    e
                )))
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(feed)
}

fn handle_attrs(
    event: &BytesStart<'_>,
    tag: &str,
    base_url: &Url,
    feed: &mut RssFeed,
    current_episode: &mut Option<RssEpisode>,
) {
    let in_episode = current_episode.is_some();
    match tag {
        "enclosure" if in_episode => {
            if let Some(episode) = current_episode.as_mut() {
                set_episode_media(
                    episode,
                    attr_value(event, &["url", "href"])
                        .and_then(|value| resolve_url(base_url, &value)),
                    attr_value(event, &["type"]),
                    attr_value(event, &["length", "fileSize", "filesize"])
                        .and_then(|value| value.parse::<u64>().ok()),
                );
            }
        }
        "media:content" if in_episode => {
            if let Some(episode) = current_episode.as_mut() {
                let media_type = attr_value(event, &["type"]);
                let medium = attr_value(event, &["medium"]);
                let looks_audio = media_type
                    .as_deref()
                    .map(|value| value.starts_with("audio/"))
                    .unwrap_or(false)
                    || medium
                        .as_deref()
                        .map(|value| value.eq_ignore_ascii_case("audio"))
                        .unwrap_or(false);
                if looks_audio || episode.enclosure_url.is_none() {
                    set_episode_media(
                        episode,
                        attr_value(event, &["url"]).and_then(|value| resolve_url(base_url, &value)),
                        media_type,
                        attr_value(event, &["fileSize", "filesize", "length"])
                            .and_then(|value| value.parse::<u64>().ok()),
                    );
                }
            }
        }
        "link" if in_episode => {
            let rel = attr_value(event, &["rel"]).unwrap_or_default();
            if rel.eq_ignore_ascii_case("enclosure") {
                if let Some(episode) = current_episode.as_mut() {
                    set_episode_media(
                        episode,
                        attr_value(event, &["href"])
                            .and_then(|value| resolve_url(base_url, &value)),
                        attr_value(event, &["type"]),
                        attr_value(event, &["length"]).and_then(|value| value.parse::<u64>().ok()),
                    );
                }
            }
        }
        "itunes:image" | "image" | "media:thumbnail" if !in_episode => {
            if feed.image.is_none() {
                feed.image = attr_value(event, &["href", "url"])
                    .and_then(|value| resolve_url(base_url, &value));
            }
        }
        "itunes:category" if !in_episode => {
            if let Some(category) =
                attr_value(event, &["text"]).and_then(|value| clean_text(&value))
            {
                push_unique(&mut feed.categories, category);
            }
        }
        _ => {}
    }
}

fn set_episode_media(
    episode: &mut RssEpisode,
    url: Option<String>,
    media_type: Option<String>,
    media_size: Option<u64>,
) {
    if let Some(url) = url {
        episode.enclosure_url = Some(url);
    }
    if media_type.is_some() {
        episode.enclosure_type = media_type;
    }
    if media_size.is_some() {
        episode.enclosure_size = media_size;
    }
}

fn handle_text(
    stack: &[String],
    raw_value: &str,
    feed: &mut RssFeed,
    current_episode: &mut Option<RssEpisode>,
) {
    let Some(tag) = stack.last().map(String::as_str) else {
        return;
    };
    let value = clean_text(raw_value);
    let in_episode = current_episode.is_some();

    if in_episode {
        let Some(episode) = current_episode.as_mut() else {
            return;
        };
        match tag {
            "title" => set_if_empty(&mut episode.title, value),
            "description" | "content:encoded" | "itunes:summary" | "summary" => {
                set_if_empty(&mut episode.description, value)
            }
            "itunes:subtitle" if episode.description.is_none() => {
                set_if_empty(&mut episode.description, value)
            }
            "guid" | "id" => set_if_empty(&mut episode.guid, value),
            "pubdate" | "published" | "updated" => {
                if let Some(value) = value {
                    episode.published_at = parse_feed_date(&value);
                    episode.pub_date = Some(value);
                }
            }
            "itunes:duration" | "duration" => {
                if let Some(value) = value {
                    episode.duration = parse_duration_seconds(&value);
                }
            }
            "itunes:episode" | "episode" => {
                if let Some(value) = value {
                    episode.episode_number = value.trim().parse::<i32>().ok();
                }
            }
            "itunes:author" | "author" | "dc:creator" => set_if_empty(&mut episode.author, value),
            _ => {}
        }
        return;
    }

    match tag {
        "title" => set_if_empty(&mut feed.title, value),
        "description" | "itunes:summary" | "summary" | "subtitle" => {
            set_if_empty(&mut feed.description, value)
        }
        "itunes:author" | "author" | "managingeditor" | "dc:creator" => {
            set_if_empty(&mut feed.author, value)
        }
        "itunes:narrator" | "narrator" => set_if_empty(&mut feed.narrator, value),
        "itunes:name"
            if stack
                .iter()
                .rev()
                .skip(1)
                .any(|parent| parent == "itunes:owner") =>
        {
            set_if_empty(&mut feed.author, value)
        }
        "category" => {
            if let Some(value) = value {
                push_unique(&mut feed.categories, value);
            }
        }
        "url" => {
            if feed.image.is_none() && stack.iter().rev().skip(1).any(|parent| parent == "image") {
                feed.image = clean_text(raw_value);
            }
        }
        _ => {}
    }
}

fn attr_value(event: &BytesStart<'_>, names: &[&str]) -> Option<String> {
    for attr in event.attributes().flatten() {
        let key = tag_name(attr.key.as_ref());
        if names.iter().any(|name| key.eq_ignore_ascii_case(name)) {
            if let Ok(value) = attr.unescape_value() {
                return Some(value.to_string());
            }
            return Some(String::from_utf8_lossy(attr.value.as_ref()).to_string());
        }
    }
    None
}

fn tag_name(value: &[u8]) -> String {
    String::from_utf8_lossy(value).to_ascii_lowercase()
}

fn resolve_url(base_url: &Url, value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Url::parse(value)
        .or_else(|_| base_url.join(value))
        .ok()
        .map(|url| url.to_string())
}

fn clean_text(value: &str) -> Option<String> {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }

    let output = output
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");
    let output = output.split_whitespace().collect::<Vec<_>>().join(" ");

    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn set_if_empty(target: &mut Option<String>, value: Option<String>) {
    if target.as_deref().unwrap_or("").trim().is_empty() {
        *target = value;
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&value))
    {
        values.push(value);
    }
}

fn normalized_categories(categories: &[String]) -> Option<String> {
    let categories: Vec<String> = categories
        .iter()
        .filter_map(|value| clean_text(value))
        .collect();
    if categories.is_empty() {
        None
    } else {
        Some(categories.join(","))
    }
}

fn parse_feed_date(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc2822(value)
        .or_else(|_| DateTime::parse_from_rfc3339(value))
        .map(|date| date.with_timezone(&Utc))
        .ok()
}

fn parse_duration_seconds(value: &str) -> Option<i32> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(seconds) = value.parse::<f64>() {
        return Some(seconds.round().max(0.0) as i32);
    }

    let parts: Vec<&str> = value.split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }

    let mut seconds = 0i32;
    for part in parts {
        seconds = seconds.checked_mul(60)?;
        seconds = seconds.checked_add(part.trim().parse::<i32>().ok()?)?;
    }
    Some(seconds.max(0))
}

fn parse_title_episode_number(value: &str) -> Option<i32> {
    let value = value.trim();
    let mut chars = value.chars().peekable();

    while matches!(chars.peek(), Some(ch) if ch.is_whitespace()) {
        chars.next();
    }

    if matches!(chars.peek(), Some('#' | '＃')) {
        chars.next();
        let digits: String = chars
            .by_ref()
            .take_while(|ch| ch.is_ascii_digit())
            .collect();
        return digits.parse::<i32>().ok();
    }

    for prefix in ["第", "EP", "Ep", "ep", "Episode ", "episode "] {
        if let Some(rest) = value.strip_prefix(prefix) {
            let digits: String = rest
                .chars()
                .skip_while(|ch| ch.is_whitespace())
                .take_while(|ch| ch.is_ascii_digit())
                .collect();
            if let Ok(number) = digits.parse::<i32>() {
                return Some(number);
            }
        }
    }

    None
}

fn hash_string(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}
