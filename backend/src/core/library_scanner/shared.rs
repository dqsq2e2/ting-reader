//! Shared scan utilities used by both local and WebDAV scanners

use std::collections::{HashMap, HashSet};
use tracing::{info, warn};

use crate::db::repository::Repository;
use super::{LibraryScanner, ScanResult};

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
        let all_books = self.book_repo.find_all_minimal_by_library(library_id)
            .await
            .unwrap_or_default();

        let mut path_map = HashMap::new();
        let mut hash_map = HashMap::new();

        for (id, path, hash, manual_corrected, match_pattern) in &all_books {
            path_map.insert(path.clone(), (id.clone(), *manual_corrected, match_pattern.clone()));
            hash_map.insert(hash.clone(), (id.clone(), *manual_corrected, match_pattern.clone()));
        }

        PrefetchedBooks { path_map, hash_map, all_books }
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

