//! Shared scan utilities used by both local and WebDAV scanners

use std::collections::{HashMap, HashSet};
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
