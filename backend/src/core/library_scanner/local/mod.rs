mod chapters;
mod extraction;
mod processing;

use super::{LibraryScanner, LocalLibraryScanContext, MetadataSource, ScanResult, ScanStatus};
use crate::core::app::error::Result;
use crate::core::library_scanner::shared::{
    ChapterRangeDir, CoalescedRangeDirs, SeriesDirectoryCandidate, infer_series_directories,
    parse_chapter_range_dir_name, select_mergeable_range_groups,
};
use crate::db::repository::Repository;
use chapters::ChapterProcessingOptions;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

struct LocalBookProcessingContext<'a> {
    library_id: &'a str,
    dir: &'a Path,
    files: &'a [PathBuf],
    last_scanned: Option<chrono::DateTime<chrono::Utc>>,
    task_id: Option<&'a str>,
    scraper_config: &'a crate::db::models::ScraperConfig,
    manual_corrected_patterns: &'a [(String, String)],
    existing_info: Option<(String, i32, Option<String>)>,
    chapter_counts: &'a HashMap<String, crate::db::repository::chapter::ChapterCounts>,
    changed_file_paths: &'a HashSet<PathBuf>,
    fallback_title_override: Option<&'a str>,
    mode: super::ScanMode,
}

#[derive(Clone)]
struct LocalFileSnapshot {
    fingerprint: String,
    modified_at: Option<String>,
    size: Option<i64>,
}

impl LibraryScanner {
    /// Scan a local library
    pub(crate) async fn scan_local_library(
        &self,
        context: LocalLibraryScanContext<'_>,
    ) -> Result<ScanResult> {
        let LocalLibraryScanContext {
            library_id,
            path,
            task_id,
            last_scanned,
            scraper_config,
            mode,
            scan_paths,
        } = context;
        let mut scan_result = ScanResult {
            start_time: Some(std::time::Instant::now()),
            ..Default::default()
        };

        self.update_progress_key(task_id, "scan.local.scanning", serde_json::json!({}))
            .await;

        // Get all supported extensions dynamically
        let supported_extensions = self.get_supported_extensions().await;
        let cached_file_states = self
            .scan_state_repo
            .find_by_library_kind(library_id, "local_file")
            .await
            .unwrap_or_default();

        // 1. Recursively find all audio files and group them by directory
        let mut dir_groups: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        let mut file_snapshots: HashMap<PathBuf, LocalFileSnapshot> = HashMap::new();
        let mut sidecar_fingerprints: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut seen_file_paths = HashSet::new();
        let mut changed_file_paths = HashSet::new();
        let mut changed_file_dirs = HashSet::new();
        let mut changed_parent_dirs = HashSet::new();
        let mut deleted_file_paths = Vec::new();

        let mut walk_errors = 0usize;
        let walk_roots: Vec<PathBuf> = collapse_local_scan_roots(match scan_paths {
            Some(paths) if !paths.is_empty() && !mode.is_full() => paths
                .iter()
                .filter(|scan_path| scan_path.starts_with(path))
                .cloned()
                .collect(),
            _ => vec![path.to_path_buf()],
        });

        for walk_root in &walk_roots {
            // A scoped watcher path can legitimately disappear after a delete
            // or rename. Treat that as an empty subtree so the snapshot diff
            // removes its stale files/books; the main library root still uses
            // the ordinary error path to avoid mass deletion on an unavailable
            // mount.
            if !mode.is_full() && scan_paths.is_some() && !walk_root.exists() {
                continue;
            }
            for entry in WalkDir::new(walk_root).follow_links(true).into_iter() {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        walk_errors += 1;
                        let error_path = e
                            .path()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| path.display().to_string());
                        warn!(
                            path = %error_path,
                            error = %e,
                            "Failed to read local library path during scan"
                        );
                        if scan_result.errors.len() < 20 {
                            scan_result
                                .errors
                                .push(format!("Failed to read {}: {}", error_path, e));
                        }
                        continue;
                    }
                };
                if !entry.file_type().is_file() {
                    continue;
                }

                let entry_path = entry.path();
                let ext = entry_path
                    .extension()
                    .map(|ext| ext.to_string_lossy().to_lowercase());
                let is_audio = ext
                    .as_ref()
                    .is_some_and(|ext| supported_extensions.contains(ext));
                let is_sidecar = is_local_scan_sidecar(entry_path);
                if !is_audio && !is_sidecar {
                    continue;
                }

                let path_buf = entry_path.to_path_buf();
                let snapshot = local_file_snapshot(&entry);
                let path_string = path_buf.to_string_lossy().to_string();
                seen_file_paths.insert(path_string.clone());
                if cached_file_states
                    .get(&path_string)
                    .map(|state| state.fingerprint != snapshot.fingerprint)
                    .unwrap_or(true)
                {
                    changed_file_paths.insert(path_buf.clone());
                    mark_changed_local_directory(
                        entry_path.parent(),
                        &mut changed_file_dirs,
                        &mut changed_parent_dirs,
                    );
                }
                if is_audio && let Some(parent) = entry_path.parent() {
                    dir_groups
                        .entry(parent.to_path_buf())
                        .or_default()
                        .push(path_buf.clone());
                }
                if is_sidecar && let Some(parent) = entry_path.parent() {
                    sidecar_fingerprints
                        .entry(parent.to_path_buf())
                        .or_default()
                        .push(snapshot.fingerprint.clone());
                }
                file_snapshots.insert(path_buf, snapshot);
            }
        }
        for state in cached_file_states.values() {
            let state_path = Path::new(&state.entry_path);
            let state_in_scope = mode.is_full()
                || walk_roots
                    .iter()
                    .any(|walk_root| state_path.starts_with(walk_root));
            if state_in_scope && !seen_file_paths.contains(&state.entry_path) {
                deleted_file_paths.push(state.entry_path.clone());
                let parent = state
                    .parent_path
                    .as_deref()
                    .map(Path::new)
                    .or_else(|| state_path.parent());
                mark_changed_local_directory(
                    parent,
                    &mut changed_file_dirs,
                    &mut changed_parent_dirs,
                );
            }
        }
        scan_result.failed_count += walk_errors;

        self.update_progress_key(
            task_id,
            "scan.audio_dirs.found",
            serde_json::json!({ "count": dir_groups.len() }),
        )
        .await;

        // 2. Process each directory group as a book
        let (dir_groups, coalesced_range_dirs) =
            coalesce_local_range_directory_groups(path, dir_groups);
        let live_book_state_paths: HashSet<String> = dir_groups
            .keys()
            .map(|dir| dir.to_string_lossy().to_string())
            .collect();
        let inferred_series = if scraper_config.auto_generate_series {
            infer_local_series_directories(path, dir_groups.keys())
        } else {
            HashMap::new()
        };
        let changed_series_keys: HashSet<(PathBuf, String)> = inferred_series
            .iter()
            .filter(|(dir, _)| {
                local_group_is_changed(
                    dir,
                    coalesced_range_dirs.get(*dir),
                    &changed_file_dirs,
                    &changed_parent_dirs,
                )
            })
            .filter_map(|(dir, series)| Some((dir.parent()?.to_path_buf(), series.title.clone())))
            .collect();
        let affected_series_dirs: HashSet<PathBuf> = inferred_series
            .iter()
            .filter(|(dir, series)| {
                dir.parent().is_some_and(|parent| {
                    changed_series_keys.contains(&(parent.to_path_buf(), series.title.clone()))
                })
            })
            .map(|(dir, _)| dir.clone())
            .collect();
        let total_groups = dir_groups.len();
        let mut processed_count = 0;

        // Pre-fetch all books (minimal) for the library to handle deletions and fast lookup
        // Returns: (id, path, hash, manual_corrected, match_pattern)
        let all_books_minimal = self
            .book_repo
            .find_all_minimal_by_library(library_id)
            .await
            .unwrap_or_default();
        let chapter_counts = if mode.is_full() {
            HashMap::new()
        } else {
            self.chapter_repo
                .count_by_library(library_id)
                .await
                .unwrap_or_default()
        };
        let scan_config_fingerprint = scraper_config_fingerprint(scraper_config);
        let cached_states = self
            .scan_state_repo
            .find_by_library_kind(library_id, "book")
            .await
            .unwrap_or_default();

        // Build lookup maps
        // Map: Path -> (id, manual_corrected, match_pattern)
        let mut book_path_map: HashMap<PathBuf, (String, i32, Option<String>)> = HashMap::new();
        let mut book_hash_map: HashMap<String, (String, i32, Option<String>)> = HashMap::new();

        for (id, path, hash, manual_corrected, match_pattern) in &all_books_minimal {
            book_path_map.insert(
                PathBuf::from(path),
                (id.clone(), *manual_corrected, match_pattern.clone()),
            );
            book_hash_map.insert(
                hash.clone(),
                (id.clone(), *manual_corrected, match_pattern.clone()),
            );
        }

        let manual_corrected_patterns: Vec<(String, String)> = all_books_minimal
            .iter()
            .filter(|(_, _, _, mc, mp)| *mc == 1 && mp.is_some())
            .map(|(id, _, _, _, mp)| (id.clone(), mp.clone().unwrap()))
            .collect();

        let mut found_book_ids: HashSet<String> = HashSet::new();
        let mut absorbed_range_book_ids: HashMap<String, String> = HashMap::new();
        let mut pending_states = Vec::new();
        let pending_file_states: Vec<crate::db::repository::LibraryScanState> = file_snapshots
            .iter()
            .filter(|(file_path, _)| changed_file_paths.contains(*file_path))
            .map(|(file_path, snapshot)| {
                let mut state = crate::db::repository::LibraryScanState::new(
                    library_id,
                    file_path.to_string_lossy(),
                    "local_file",
                    &snapshot.fingerprint,
                );
                state.modified_at = snapshot.modified_at.clone();
                state.size = snapshot.size;
                state.parent_path = file_path
                    .parent()
                    .map(|parent| parent.to_string_lossy().to_string());
                state
            })
            .collect();

        for (dir, mut files) in dir_groups {
            processed_count += 1;
            // Progress/cancellation updates are database operations. Keep them
            // periodic so a large scan does not turn into one DB round-trip per
            // directory while still remaining responsive to cancellation.
            let report_progress = processed_count == 1
                || processed_count % 16 == 0
                || processed_count == total_groups;
            if report_progress {
                self.check_cancellation(task_id).await?;
            }
            let dir_name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown");

            if report_progress {
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
            }

            // Optimization: Find existing book to avoid DB lookup
            let mut existing_info = book_path_map.get(&dir).cloned();

            // If not found by path, try hash (for moved books)
            if existing_info.is_none() {
                let book_hash = self.generate_book_hash(&dir);
                existing_info = book_hash_map.get(&book_hash).cloned();
            }

            if existing_info.is_none()
                && let Some(child_dirs) = coalesced_range_dirs.get(&dir)
            {
                for child_dir in &child_dirs.child_dirs {
                    if let Some(info) = book_path_map.get(child_dir).cloned() {
                        existing_info = Some(info);
                        break;
                    }
                }
            }

            let dir_path_string = dir.to_string_lossy().to_string();
            let files_changed = local_group_is_changed(
                &dir,
                coalesced_range_dirs.get(&dir),
                &changed_file_dirs,
                &changed_parent_dirs,
            );
            let existing_lock_state = existing_info
                .as_ref()
                .map(|(_, manual_corrected, _)| *manual_corrected)
                .unwrap_or(0);
            let state_config_fingerprint =
                format!("{}:lock={}", scan_config_fingerprint, existing_lock_state);
            let config_changed = cached_states.get(&dir_path_string).is_none_or(|state| {
                state.config_fingerprint.as_deref() != Some(state_config_fingerprint.as_str())
            });
            if let Some((book_id, _, _)) = existing_info.as_ref()
                && chapter_counts.get(book_id).is_some_and(|counts| {
                    counts.missing_duration == 0
                        && !processing::embedded_chapters_pending(&files, counts)
                })
                && cached_states.get(&dir_path_string).is_some_and(|state| {
                    !files_changed
                        && state.config_fingerprint.as_deref()
                            == Some(state_config_fingerprint.as_str())
                })
            {
                scan_result.total_books += 1;
                scan_result.books_skipped += 1;
                found_book_ids.insert(book_id.clone());
                if affected_series_dirs.contains(&dir)
                    && let Some(series_info) = inferred_series.get(&dir)
                    && let Err(e) = self
                        .link_book_to_inferred_series(library_id, book_id, series_info)
                        .await
                {
                    warn!(
                        path = ?dir,
                        book_id = %book_id,
                        error = %e,
                        "Failed to refresh affected inferred series member"
                    );
                }
                continue;
            }

            // Only directories in the changed set pay the natural-sort and
            // aggregate-fingerprint cost. The discovery pass itself stays O(N).
            files.sort_by_cached_key(|path| {
                crate::core::books::text_cleaner::chapter_order_key(&path.to_string_lossy())
            });
            let dir_fingerprint = cached_states
                .get(&dir_path_string)
                .filter(|_| !files_changed)
                .map(|state| state.fingerprint.clone())
                .unwrap_or_else(|| {
                    local_directory_fingerprint(
                        &dir,
                        &files,
                        &file_snapshots,
                        &sidecar_fingerprints,
                    )
                });

            match self
                .process_book_directory(LocalBookProcessingContext {
                    library_id,
                    dir: &dir,
                    files: &files,
                    last_scanned,
                    task_id,
                    scraper_config,
                    manual_corrected_patterns: &manual_corrected_patterns,
                    existing_info,
                    chapter_counts: &chapter_counts,
                    changed_file_paths: &changed_file_paths,
                    fallback_title_override: coalesced_range_dirs
                        .get(&dir)
                        .and_then(|range_dirs| range_dirs.title_override.as_deref()),
                    mode: if config_changed {
                        super::ScanMode::Full
                    } else {
                        mode
                    },
                })
                .await
            {
                Ok((book_id, status)) => {
                    scan_result.total_books += 1;
                    match status {
                        ScanStatus::Created => scan_result.books_created += 1,
                        ScanStatus::Updated => scan_result.books_updated += 1,
                        ScanStatus::Skipped => scan_result.books_skipped += 1,
                    }
                    if status != ScanStatus::Skipped {
                        scan_result.changed_book_ids.insert(book_id.clone());
                    }
                    found_book_ids.insert(book_id.clone());
                    let mut state = crate::db::repository::LibraryScanState::new(
                        library_id,
                        dir_path_string,
                        "book",
                        dir_fingerprint,
                    );
                    state.config_fingerprint = Some(state_config_fingerprint);
                    pending_states.push(state);
                    if status != ScanStatus::Skipped
                        && let Some(series_info) = inferred_series.get(&dir)
                        && let Err(e) = self
                            .link_book_to_inferred_series(library_id, &book_id, series_info)
                            .await
                    {
                        warn!(
                            path = ?dir,
                            book_id = %book_id,
                            error = %e,
                            "Failed to link book to inferred series"
                        );
                    }
                    if status != ScanStatus::Skipped
                        && let Some(child_dirs) = coalesced_range_dirs.get(&dir)
                    {
                        for child_dir in &child_dirs.child_dirs {
                            if let Some((child_book_id, manual_corrected, _)) =
                                book_path_map.get(child_dir)
                                && child_book_id != &book_id
                                && *manual_corrected == 0
                            {
                                absorbed_range_book_ids
                                    .insert(child_book_id.clone(), book_id.clone());
                            }
                        }
                    }
                    debug!(book_id = %book_id, path = ?dir, status = ?status, "Processed book directory");
                }
                Err(e) => {
                    scan_result.failed_count += 1;
                    warn!(path = ?dir, error = %e, "Failed to process book directory");
                    scan_result
                        .errors
                        .push(format!("Failed to process {}: {}", dir.display(), e));
                }
            }

            if processed_count % 25 == 0 {
                self.plugin_manager.garbage_collect_all().await;
            }
        }

        let mut deleted_book_state_paths = Vec::new();
        for (source_id, target_id) in absorbed_range_book_ids {
            if found_book_ids.contains(&source_id) {
                continue;
            }

            if let Some(merge_service) = &self.merge_service {
                if let Err(e) = merge_service
                    .absorb_scanned_book(&target_id, &source_id)
                    .await
                {
                    warn!(
                        "Failed to absorb range-segment book {} into {}: {}",
                        source_id, target_id, e
                    );
                } else {
                    if let Some((_, source_path, _, _, _)) = all_books_minimal
                        .iter()
                        .find(|(id, _, _, _, _)| id == &source_id)
                    {
                        deleted_book_state_paths.push(source_path.clone());
                    }
                    scan_result.books_deleted += 1;
                }
            } else {
                info!(
                    "Deleting range-segment book record after merging into parent book: {}",
                    source_id
                );
                if let Err(e) = self.book_repo.delete(&source_id).await {
                    warn!(
                        "Failed to delete absorbed range-segment book {}: {}",
                        source_id, e
                    );
                } else {
                    if let Some((_, source_path, _, _, _)) = all_books_minimal
                        .iter()
                        .find(|(id, _, _, _, _)| id == &source_id)
                    {
                        deleted_book_state_paths.push(source_path.clone());
                    }
                    scan_result.books_deleted += 1;
                    if let Err(e) = self.chapter_repo.delete_by_book(&source_id).await {
                        warn!(
                            "Failed to delete chapters for absorbed range-segment book {}: {}",
                            source_id, e
                        );
                    }
                }
            }
        }

        // 3. Handle deletions. A scoped watcher scan is allowed to remove
        // missing books only inside the affected directories; an ordinary
        // incremental scan still avoids deletion because it did not inspect
        // the complete library.
        let complete_library_listing =
            mode.is_full() || scan_paths.is_none_or(|paths| paths.is_empty());
        if walk_errors == 0
            && (complete_library_listing || scan_paths.is_some_and(|paths| !paths.is_empty()))
        {
            for (id, path_str, _, _, _) in all_books_minimal {
                if !found_book_ids.contains(&id) {
                    let path = Path::new(&path_str);
                    let in_scope = complete_library_listing
                        || scan_paths
                            .map(|paths| paths.iter().any(|root| path.starts_with(root)))
                            .unwrap_or(false);
                    if in_scope {
                        deleted_book_state_paths.push(path_str.clone());
                        info!("Book path missing, deleting record: {}", path_str);
                        if let Err(e) = self.book_repo.delete(&id).await {
                            warn!("Failed to delete missing book {}: {}", id, e);
                        } else {
                            scan_result.books_deleted += 1;
                            if let Err(e) = self.chapter_repo.delete_by_book(&id).await {
                                warn!("Failed to delete chapters for missing book {}: {}", id, e);
                            }
                        }
                    }
                }
            }
        }

        if mode.is_full() && walk_errors == 0 {
            deleted_book_state_paths.extend(
                cached_states
                    .keys()
                    .filter(|entry_path| !live_book_state_paths.contains(*entry_path))
                    .cloned(),
            );
        }
        if walk_errors == 0 {
            self.scan_state_repo
                .delete_many(library_id, "book", deleted_book_state_paths)
                .await?;
            self.scan_state_repo
                .delete_many(library_id, "local_file", deleted_file_paths)
                .await?;
        }
        self.scan_state_repo.upsert_many(pending_states).await?;
        self.scan_state_repo
            .upsert_many(pending_file_states)
            .await?;
        if complete_library_listing && walk_errors == 0 {
            let baseline = crate::db::repository::LibraryScanState::new(
                library_id,
                path.to_string_lossy(),
                "local_baseline",
                "complete",
            );
            self.scan_state_repo.upsert_many(vec![baseline]).await?;
        }

        // Final garbage collection after scan
        self.plugin_manager.garbage_collect_all().await;

        Ok(scan_result)
    }
}

fn local_directory_fingerprint(
    dir: &Path,
    files: &[PathBuf],
    file_snapshots: &HashMap<PathBuf, LocalFileSnapshot>,
    sidecar_fingerprints: &HashMap<PathBuf, Vec<String>>,
) -> String {
    let mut hasher = Sha256::new();
    let mut entries: Vec<String> = files
        .iter()
        .filter_map(|path| file_snapshots.get(path))
        .map(|snapshot| snapshot.fingerprint.clone())
        .collect();
    entries.extend(sidecar_fingerprints.get(dir).into_iter().flatten().cloned());
    entries.sort();
    for entry in entries {
        hasher.update(entry.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn collapse_local_scan_roots(mut roots: Vec<PathBuf>) -> Vec<PathBuf> {
    roots.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });

    let mut collapsed = Vec::with_capacity(roots.len());
    for root in roots {
        if collapsed
            .iter()
            .any(|existing: &PathBuf| root.starts_with(existing))
        {
            continue;
        }
        collapsed.push(root);
    }
    collapsed
}

fn local_file_snapshot(entry: &walkdir::DirEntry) -> LocalFileSnapshot {
    let metadata = entry.metadata().ok();
    let size = metadata
        .as_ref()
        .map(|metadata| metadata.len() as i64)
        .unwrap_or_default();
    let modified_nanos = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    LocalFileSnapshot {
        fingerprint: format!(
            "{}:{}:{}",
            entry.path().to_string_lossy(),
            size,
            modified_nanos
        ),
        modified_at: if modified_nanos == 0 {
            None
        } else {
            Some(modified_nanos.to_string())
        },
        size: Some(size),
    }
}

fn is_local_scan_sidecar(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    name == "metadata.json"
        || name == "book.nfo"
        || name.starts_with("cover.")
        || name.starts_with("folder.")
}

/// Mark the directory containing a changed file and all of its ancestors.
///
/// The containing directory is the unit that needs book reconciliation. The
/// ancestor set is kept separately so a coalesced range/series directory can
/// cheaply detect that one of its children changed without marking siblings.
fn mark_changed_local_directory(
    parent: Option<&Path>,
    changed_dirs: &mut HashSet<PathBuf>,
    changed_ancestors: &mut HashSet<PathBuf>,
) {
    let Some(parent) = parent else {
        return;
    };

    changed_dirs.insert(parent.to_path_buf());
    let mut current = parent;
    while let Some(ancestor) = current.parent() {
        if !changed_ancestors.insert(ancestor.to_path_buf()) {
            break;
        }
        current = ancestor;
    }
}

fn local_group_is_changed(
    dir: &Path,
    coalesced: Option<&CoalescedRangeDirs<PathBuf>>,
    changed_dirs: &HashSet<PathBuf>,
    changed_ancestors: &HashSet<PathBuf>,
) -> bool {
    if changed_dirs.contains(dir) || changed_ancestors.contains(dir) {
        return true;
    }

    coalesced.is_some_and(|range_dirs| {
        range_dirs.child_dirs.iter().any(|child_dir| {
            changed_dirs.contains(child_dir) || changed_ancestors.contains(child_dir)
        })
    })
}

fn scraper_config_fingerprint(config: &crate::db::models::ScraperConfig) -> String {
    let serialized = serde_json::to_vec(config).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(serialized);
    format!("{:x}", hasher.finalize())
}

fn coalesce_local_range_directory_groups(
    root: &Path,
    mut dir_groups: HashMap<PathBuf, Vec<PathBuf>>,
) -> (
    HashMap<PathBuf, Vec<PathBuf>>,
    HashMap<PathBuf, CoalescedRangeDirs<PathBuf>>,
) {
    let mut candidates: HashMap<PathBuf, Vec<(PathBuf, ChapterRangeDir)>> = HashMap::new();

    for dir in dir_groups.keys() {
        let Some(parent) = dir.parent() else {
            continue;
        };
        let Some(dir_name) = dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(range_dir) = parse_chapter_range_dir_name(dir_name) else {
            continue;
        };

        candidates
            .entry(parent.to_path_buf())
            .or_default()
            .push((dir.clone(), range_dir));
    }

    let mut coalesced_range_dirs = HashMap::new();

    for (parent, entries) in candidates {
        let parent_name = parent
            .file_name()
            .and_then(|name| name.to_str())
            .or_else(|| root.file_name().and_then(|name| name.to_str()))
            .unwrap_or("");

        let ranges: Vec<ChapterRangeDir> = entries.iter().map(|(_, range)| range.clone()).collect();

        for group in select_mergeable_range_groups(parent_name, &ranges) {
            let mut selected: Vec<(PathBuf, ChapterRangeDir)> = group
                .indices
                .into_iter()
                .map(|index| entries[index].clone())
                .collect();
            selected.sort_by_key(|(_, range)| (range.start, range.end));

            let Some(first_child_dir) = selected.first().map(|(child_dir, _)| child_dir.clone())
            else {
                continue;
            };

            let target_dir = if group.merge_into_parent {
                parent.clone()
            } else {
                first_child_dir
            };

            let child_dirs: Vec<PathBuf> = selected
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

fn infer_local_series_directories<'a>(
    root: &Path,
    dirs: impl Iterator<Item = &'a PathBuf>,
) -> HashMap<PathBuf, crate::core::library_scanner::shared::InferredSeriesInfo> {
    let candidates: Vec<SeriesDirectoryCandidate<PathBuf>> = dirs
        .filter_map(|dir| {
            let parent = dir.parent()?;
            let name = dir.file_name()?.to_str()?;
            let parent_name = parent
                .file_name()
                .and_then(|value| value.to_str())
                .or_else(|| root.file_name().and_then(|value| value.to_str()))
                .unwrap_or("");

            Some(SeriesDirectoryCandidate {
                key: dir.clone(),
                parent_key: parent.to_string_lossy().to_string(),
                parent_name: parent_name.to_string(),
                name: name.to_string(),
            })
        })
        .collect();

    infer_series_directories(&candidates)
}

#[cfg(test)]
#[path = "../../../../tests/unit/core/library_scanner/local.rs"]
mod tests;
