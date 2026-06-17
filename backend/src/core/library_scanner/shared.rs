//! Shared scan utilities used by both local and WebDAV scanners

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::OnceLock;
use tracing::{info, warn};

use super::{LibraryScanner, ScanResult};
use crate::db::repository::Repository;
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterRangeDir {
    pub start: u32,
    pub end: u32,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InferredSeriesInfo {
    pub title: String,
    pub order: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct SeriesDirectoryCandidate<K> {
    pub key: K,
    pub parent_key: String,
    pub parent_name: String,
    pub name: String,
}

#[derive(Debug, Clone)]
struct ParsedSeriesDirectory {
    base_title: Option<String>,
    order: Option<i32>,
}

/// Pre-fetched book lookup data
pub struct PrefetchedBooks {
    /// Map: path -> (id, manual_corrected, match_pattern)
    pub path_map: HashMap<String, (String, i32, Option<String>)>,
    /// Map: hash -> (id, manual_corrected, match_pattern)
    pub hash_map: HashMap<String, (String, i32, Option<String>)>,
    /// All minimal book records
    pub all_books: Vec<(String, String, String, i32, Option<String>)>,
}

impl LibraryScanner {
    /// Pre-fetch all existing books for a library and build lookup maps
    pub(crate) async fn prefetch_books(&self, library_id: &str) -> PrefetchedBooks {
        let all_books = self
            .book_repo
            .find_all_minimal_by_library(library_id)
            .await
            .unwrap_or_default();

        let mut path_map = HashMap::new();
        let mut hash_map = HashMap::new();

        for (id, path, hash, manual_corrected, match_pattern) in &all_books {
            path_map.insert(
                path.clone(),
                (id.clone(), *manual_corrected, match_pattern.clone()),
            );
            hash_map.insert(
                hash.clone(),
                (id.clone(), *manual_corrected, match_pattern.clone()),
            );
        }

        PrefetchedBooks {
            path_map,
            hash_map,
            all_books,
        }
    }

    /// Handle deletion of books not found during scan
    pub(crate) async fn handle_book_deletions(
        &self,
        scan_result: &mut ScanResult,
        prefetched: &PrefetchedBooks,
        found_book_ids: &HashSet<String>,
        check_path_exists: bool,
    ) {
        for (id, path_str, _, _, _) in &prefetched.all_books {
            if found_book_ids.contains(id) {
                continue;
            }

            if check_path_exists {
                let path = std::path::Path::new(path_str);
                if path.exists() {
                    continue;
                }
            }

            info!("Book missing, deleting record: {}", path_str);
            if let Err(e) = self.book_repo.delete(id).await {
                warn!("Failed to delete missing book {}: {}", id, e);
            } else {
                scan_result.books_deleted += 1;
                if let Err(e) = self.chapter_repo.delete_by_book(id).await {
                    warn!("Failed to delete chapters for missing book {}: {}", id, e);
                }
            }
        }
    }

    pub(crate) async fn link_book_to_inferred_series(
        &self,
        library_id: &str,
        book_id: &str,
        series_info: &InferredSeriesInfo,
    ) -> crate::core::error::Result<()> {
        let title = series_info.title.trim();
        if title.is_empty() {
            return Ok(());
        }

        let Some(book) = self.book_repo.find_by_id(book_id).await? else {
            return Ok(());
        };

        let new_series = crate::db::models::Series {
            id: uuid::Uuid::new_v4().to_string(),
            library_id: library_id.to_string(),
            title: title.to_string(),
            author: book.author.clone(),
            narrator: book.narrator.clone(),
            cover_url: book.cover_url.clone(),
            description: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        let series = self.series_repo.find_or_create_by_title(new_series).await?;
        let books = self.series_repo.find_books_by_series(&series.id).await?;
        let target_order = series_info.order.max(1);

        if let Some((_, current_order)) = books.iter().find(|(b, _)| b.id == book_id) {
            if *current_order != target_order {
                self.series_repo
                    .add_book(crate::db::models::SeriesBook {
                        series_id: series.id,
                        book_id: book_id.to_string(),
                        book_order: target_order,
                    })
                    .await?;
            }
        } else {
            self.series_repo
                .add_book(crate::db::models::SeriesBook {
                    series_id: series.id,
                    book_id: book_id.to_string(),
                    book_order: target_order,
                })
                .await?;
        }

        Ok(())
    }
}

pub(crate) fn infer_series_directories<K>(
    candidates: &[SeriesDirectoryCandidate<K>],
) -> HashMap<K, InferredSeriesInfo>
where
    K: Clone + Eq + Hash,
{
    let mut grouped: HashMap<
        (String, String),
        Vec<(&SeriesDirectoryCandidate<K>, ParsedSeriesDirectory, String)>,
    > = HashMap::new();

    for candidate in candidates {
        let Some(parsed) = parse_series_directory_name(&candidate.name) else {
            continue;
        };

        let title = parsed
            .base_title
            .as_deref()
            .map(clean_series_base_title)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| clean_series_base_title(&candidate.parent_name));

        if title.is_empty() {
            continue;
        }

        let key = (
            candidate.parent_key.clone(),
            canonicalize_series_title_key(&title),
        );
        grouped
            .entry(key)
            .or_default()
            .push((candidate, parsed, title));
    }

    let mut inferred = HashMap::new();

    for (_, mut entries) in grouped {
        if entries.len() < 2 {
            continue;
        }

        entries.sort_by(|a, b| match (a.1.order, b.1.order) {
            (Some(left), Some(right)) => left
                .cmp(&right)
                .then_with(|| natord::compare(&a.0.name, &b.0.name)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => natord::compare(&a.0.name, &b.0.name),
        });

        let series_title = entries
            .iter()
            .find_map(|(_, parsed, title)| {
                parsed
                    .base_title
                    .as_ref()
                    .filter(|base| !base.trim().is_empty())
                    .map(|_| title.clone())
            })
            .unwrap_or_else(|| entries[0].2.clone());

        for (index, (candidate, parsed, _)) in entries.iter().enumerate() {
            let order = parsed.order.unwrap_or((index + 1) as i32).max(1);
            inferred.insert(
                candidate.key.clone(),
                InferredSeriesInfo {
                    title: series_title.clone(),
                    order,
                },
            );
        }
    }

    inferred
}

fn parse_series_directory_name(name: &str) -> Option<ParsedSeriesDirectory> {
    let normalized = normalize_series_dir_name(name);

    if let Some(caps) = series_volume_regex().captures(&normalized) {
        let base = caps
            .name("base")
            .map(|m| clean_series_base_title(m.as_str()))
            .filter(|value| !value.is_empty());
        let num = caps
            .name("num2")
            .or_else(|| caps.name("num"))
            .and_then(|m| parse_series_number(m.as_str()));
        return Some(ParsedSeriesDirectory {
            base_title: base,
            order: num,
        });
    }

    if let Some(caps) = series_s_regex().captures(&normalized) {
        let base = caps
            .name("base")
            .map(|m| clean_series_base_title(m.as_str()))
            .filter(|value| !value.is_empty());
        let num = caps
            .name("num")
            .and_then(|m| parse_series_number(m.as_str()));
        return Some(ParsedSeriesDirectory {
            base_title: base,
            order: num,
        });
    }

    if let Some(caps) = series_word_regex().captures(&normalized) {
        let base = caps
            .name("base")
            .map(|m| clean_series_base_title(m.as_str()))
            .filter(|value| !value.is_empty());
        let num = caps
            .name("num")
            .and_then(|m| parse_series_number(m.as_str()));
        return Some(ParsedSeriesDirectory {
            base_title: base,
            order: num,
        });
    }

    if let Some((base, suffix)) = normalized.split_once('之') {
        let base = clean_series_base_title(base);
        if !base.is_empty() && !suffix.trim().is_empty() {
            return Some(ParsedSeriesDirectory {
                base_title: Some(base),
                order: parse_leading_series_number(suffix),
            });
        }
    }

    None
}

fn series_volume_regex() -> &'static Regex {
    static VOLUME_RE: OnceLock<Regex> = OnceLock::new();
    VOLUME_RE.get_or_init(|| {
        Regex::new(
            r"(?i)^\s*(?P<base>.*?)\s*(?:第\s*)?(?P<num>[0-9]+|[零〇一二两三四五六七八九十百千万]+)(?:\s*[\(（]\s*(?P<num2>[0-9]+|[零〇一二两三四五六七八九十百千万]+)\s*[\)）])?\s*(?:卷|季|部|册|輯|辑)\s*$",
        )
        .unwrap()
    })
}

fn series_s_regex() -> &'static Regex {
    static S_RE: OnceLock<Regex> = OnceLock::new();
    S_RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(?P<base>.*?)\s*[-_. ]*s(?P<num>[0-9]{1,3})\s*$").unwrap()
    })
}

fn series_word_regex() -> &'static Regex {
    static WORD_RE: OnceLock<Regex> = OnceLock::new();
    WORD_RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(?P<base>.+?)\s*[-_. ]*(?:vol(?:ume)?|season|book)\s*\.?\s*(?P<num>[0-9]{1,3})\s*$")
            .unwrap()
    })
}

fn normalize_series_dir_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '０'..='９' => {
                let digit = (ch as u32 - '０' as u32) as u8 + b'0';
                normalized.push(digit as char);
            }
            '（' => normalized.push('('),
            '）' => normalized.push(')'),
            '　' => normalized.push(' '),
            _ => normalized.push(ch),
        }
    }
    normalized.trim().to_string()
}

fn clean_series_base_title(value: &str) -> String {
    value
        .trim()
        .trim_matches(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '-' | '_' | '.' | '·' | ':' | '：' | '|' | '/' | '\\' | '(' | ')' | '（' | '）'
                )
        })
        .trim()
        .to_string()
}

fn canonicalize_series_title_key(value: &str) -> String {
    normalize_series_dir_name(value)
        .chars()
        .filter(|ch| !ch.is_whitespace() && !matches!(ch, '-' | '_' | '.' | '·' | ':' | '：'))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn parse_leading_series_number(value: &str) -> Option<i32> {
    let trimmed = value.trim_start();
    let mut token = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() || "零〇一二两三四五六七八九十百千万".contains(ch) {
            token.push(ch);
        } else if token.is_empty() && ch == '第' {
            continue;
        } else {
            break;
        }
    }

    if token.is_empty() {
        None
    } else {
        parse_series_number(&token)
    }
}

fn parse_series_number(value: &str) -> Option<i32> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if value.chars().all(|ch| ch.is_ascii_digit()) {
        return value.parse::<i32>().ok().filter(|num| *num > 0);
    }

    parse_chinese_series_number(value)
}

fn parse_chinese_series_number(value: &str) -> Option<i32> {
    let mut total = 0;
    let mut section = 0;
    let mut number = 0;
    let mut seen = false;

    for ch in value.chars() {
        let digit = match ch {
            '零' | '〇' => {
                seen = true;
                number = 0;
                continue;
            }
            '一' => Some(1),
            '二' | '两' => Some(2),
            '三' => Some(3),
            '四' => Some(4),
            '五' => Some(5),
            '六' => Some(6),
            '七' => Some(7),
            '八' => Some(8),
            '九' => Some(9),
            _ => None,
        };

        if let Some(value) = digit {
            seen = true;
            number = value;
            continue;
        }

        let unit = match ch {
            '十' => Some(10),
            '百' => Some(100),
            '千' => Some(1000),
            '万' => Some(10000),
            _ => None,
        }?;

        seen = true;
        if unit == 10000 {
            section += number;
            if section == 0 {
                section = 1;
            }
            total += section * unit;
            section = 0;
        } else {
            if number == 0 {
                number = 1;
            }
            section += number * unit;
        }
        number = 0;
    }

    if !seen {
        return None;
    }

    let result = total + section + number;
    if result > 0 {
        Some(result)
    } else {
        None
    }
}

pub(crate) fn parse_chapter_range_dir_name(name: &str) -> Option<ChapterRangeDir> {
    let normalized = normalize_range_name(name);
    let mut matches = range_regex().captures_iter(&normalized);
    let caps = matches.next()?;
    if matches.next().is_some() {
        return None;
    }

    let full_match = caps.get(0)?;
    let start = caps.get(1)?.as_str().parse::<u32>().ok()?;
    let end = caps.get(2)?.as_str().parse::<u32>().ok()?;

    if end <= start || end > 999_999 {
        return None;
    }

    // Avoid common date folder names such as "2024-2025".
    if (1900..=2100).contains(&start) && (1900..=2100).contains(&end) && end - start <= 20 {
        return None;
    }

    let suffix = &normalized[full_match.end()..];
    if starts_with_numeric_range_tail(suffix) {
        return None;
    }

    let surrounding_text = format!("{}{}", &normalized[..full_match.start()], suffix);
    let context = canonicalize_range_context(&surrounding_text);

    Some(ChapterRangeDir {
        start,
        end,
        context,
    })
}

pub(crate) fn select_mergeable_range_group(
    parent_name: &str,
    ranges: &[ChapterRangeDir],
) -> Option<Vec<usize>> {
    let mut by_context: HashMap<&str, Vec<usize>> = HashMap::new();

    for (index, range) in ranges.iter().enumerate() {
        by_context
            .entry(range.context.as_str())
            .or_default()
            .push(index);
    }

    let mut mergeable_groups = Vec::new();
    for indices in by_context.values() {
        let group: Vec<ChapterRangeDir> =
            indices.iter().map(|index| ranges[*index].clone()).collect();
        if is_mergeable_range_group(parent_name, &group) {
            mergeable_groups.push(indices.clone());
        }
    }

    if mergeable_groups.len() == 1 {
        mergeable_groups.pop()
    } else {
        None
    }
}

fn is_mergeable_range_group(parent_name: &str, ranges: &[ChapterRangeDir]) -> bool {
    if ranges.len() < 2 {
        return false;
    }

    let context = ranges[0].context.as_str();
    if ranges.iter().any(|range| range.context != context) {
        return false;
    }

    if !context.is_empty() {
        let parent_context = canonicalize_range_context(parent_name);
        if parent_context.is_empty()
            || (!parent_context.contains(context) && !context.contains(&parent_context))
        {
            return false;
        }
    }

    let mut sorted = ranges.to_vec();
    sorted.sort_by_key(|range| (range.start, range.end));

    for pair in sorted.windows(2) {
        let previous = &pair[0];
        let current = &pair[1];

        if current.start <= previous.start {
            return false;
        }
        if current.end <= previous.end {
            return false;
        }
        if current.start < previous.end {
            return false;
        }
    }

    true
}

fn range_regex() -> &'static Regex {
    static RANGE_RE: OnceLock<Regex> = OnceLock::new();
    RANGE_RE.get_or_init(|| Regex::new(r"(\d{1,6})\s*-\s*(\d{1,6})").unwrap())
}

fn normalize_range_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());

    for ch in name.chars() {
        match ch {
            '０'..='９' => {
                let digit = (ch as u32 - '０' as u32) as u8 + b'0';
                normalized.push(digit as char);
            }
            '－' | '‐' | '‑' | '‒' | '–' | '—' | '―' | '~' | '～' | '至' | '到' => {
                normalized.push('-');
            }
            _ => {
                for lower in ch.to_lowercase() {
                    normalized.push(lower);
                }
            }
        }
    }

    normalized
}

fn canonicalize_range_context(text: &str) -> String {
    let mut normalized = normalize_range_name(text);
    for token in [
        "episodes", "episode", "chapters", "chapter", "parts", "part", "tracks", "track", "volume",
        "vol", "chap", "ep", "pt", "第", "集", "章", "回", "话", "節", "节", "卷", "部", "篇",
        "讲", "講", "播",
    ] {
        normalized = normalized.replace(token, "");
    }

    normalized
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn starts_with_numeric_range_tail(text: &str) -> bool {
    let trimmed = text.trim_start();
    let Some(after_dash) = trimmed.strip_prefix('-') else {
        return false;
    };

    after_dash
        .trim_start()
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
}

#[cfg(test)]
mod inferred_series_tests {
    use super::{infer_series_directories, SeriesDirectoryCandidate};

    #[test]
    fn infers_series_from_sibling_volume_directories() {
        let candidates = vec![
            SeriesDirectoryCandidate {
                key: "root/book/season1".to_string(),
                parent_key: "root/book".to_string(),
                parent_name: "book".to_string(),
                name: "\u{4e00}\u{5ff5}\u{6c38}\u{6052}\u{7b2c}\u{4e00}\u{5b63}".to_string(),
            },
            SeriesDirectoryCandidate {
                key: "root/book/season2".to_string(),
                parent_key: "root/book".to_string(),
                parent_name: "book".to_string(),
                name: "\u{4e00}\u{5ff5}\u{6c38}\u{6052}\u{7b2c}\u{4e8c}\u{5b63}".to_string(),
            },
        ];

        let inferred = infer_series_directories(&candidates);
        assert_eq!(
            inferred["root/book/season1"].title,
            "\u{4e00}\u{5ff5}\u{6c38}\u{6052}"
        );
        assert_eq!(inferred["root/book/season1"].order, 1);
        assert_eq!(inferred["root/book/season2"].order, 2);
    }

    #[test]
    fn infers_series_from_zhi_separator() {
        let candidates = vec![
            SeriesDirectoryCandidate {
                key: "root/book/part1".to_string(),
                parent_key: "root/book".to_string(),
                parent_name: "book".to_string(),
                name: "\u{51e1}\u{4eba}\u{4fee}\u{4ed9}\u{4f20}\u{4e4b}\u{9b54}\u{9053}\u{4e89}\u{950b}".to_string(),
            },
            SeriesDirectoryCandidate {
                key: "root/book/part2".to_string(),
                parent_key: "root/book".to_string(),
                parent_name: "book".to_string(),
                name: "\u{51e1}\u{4eba}\u{4fee}\u{4ed9}\u{4f20}\u{4e4b}\u{521d}\u{5165}\u{661f}\u{6d77}".to_string(),
            },
        ];

        let inferred = infer_series_directories(&candidates);
        assert_eq!(
            inferred["root/book/part1"].title,
            "\u{51e1}\u{4eba}\u{4fee}\u{4ed9}\u{4f20}"
        );
        let mut orders = vec![
            inferred["root/book/part1"].order,
            inferred["root/book/part2"].order,
        ];
        orders.sort();
        assert_eq!(orders, vec![1, 2]);
    }

    #[test]
    fn infers_series_from_parent_when_only_season_code_is_present() {
        let candidates = vec![
            SeriesDirectoryCandidate {
                key: "root/book/s01".to_string(),
                parent_key: "root/book".to_string(),
                parent_name: "\u{4e09}\u{4f53}".to_string(),
                name: "S01".to_string(),
            },
            SeriesDirectoryCandidate {
                key: "root/book/s02".to_string(),
                parent_key: "root/book".to_string(),
                parent_name: "\u{4e09}\u{4f53}".to_string(),
                name: "S02".to_string(),
            },
        ];

        let inferred = infer_series_directories(&candidates);
        assert_eq!(inferred["root/book/s01"].title, "\u{4e09}\u{4f53}");
        assert_eq!(inferred["root/book/s01"].order, 1);
        assert_eq!(inferred["root/book/s02"].order, 2);
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_chapter_range_dir_name, select_mergeable_range_group, ChapterRangeDir};

    #[test]
    fn parses_common_chapter_range_directory_names() {
        let simple = parse_chapter_range_dir_name("1- 500").unwrap();
        assert_eq!(simple.start, 1);
        assert_eq!(simple.end, 500);
        assert_eq!(simple.context, "");

        let decorated = parse_chapter_range_dir_name("第001 - 050集").unwrap();
        assert_eq!(decorated.start, 1);
        assert_eq!(decorated.end, 50);
        assert_eq!(decorated.context, "");

        let tilde = parse_chapter_range_dir_name("[0501～1000]").unwrap();
        assert_eq!(tilde.start, 501);
        assert_eq!(tilde.end, 1000);
    }

    #[test]
    fn ignores_non_chapter_ranges() {
        assert!(parse_chapter_range_dir_name("2024-2025").is_none());
        assert!(parse_chapter_range_dir_name("500-1").is_none());
        assert!(parse_chapter_range_dir_name("1-50-100").is_none());
    }

    #[test]
    fn selects_only_safe_sibling_range_groups() {
        let ranges = vec![
            parse_chapter_range_dir_name("001-050").unwrap(),
            parse_chapter_range_dir_name("051-100").unwrap(),
        ];
        assert_eq!(
            select_mergeable_range_group("书名", &ranges),
            Some(vec![0, 1])
        );

        let prefixed = vec![
            parse_chapter_range_dir_name("书名 001-050").unwrap(),
            parse_chapter_range_dir_name("书名 051-100").unwrap(),
        ];
        assert_eq!(
            select_mergeable_range_group("书名", &prefixed),
            Some(vec![0, 1])
        );

        let different_books = vec![
            parse_chapter_range_dir_name("书A 001-050").unwrap(),
            parse_chapter_range_dir_name("书B 051-100").unwrap(),
        ];
        assert_eq!(select_mergeable_range_group("合集", &different_books), None);
    }

    #[test]
    fn rejects_overlapping_ranges() {
        let ranges = vec![
            ChapterRangeDir {
                start: 1,
                end: 100,
                context: String::new(),
            },
            ChapterRangeDir {
                start: 80,
                end: 150,
                context: String::new(),
            },
        ];

        assert_eq!(select_mergeable_range_group("书名", &ranges), None);
    }
}
