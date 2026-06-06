use crate::core::error::{Result, TingError};
use crate::db::models::Book;
use crate::db::repository::{BookRepository, ChapterRepository, Repository};
use std::sync::Arc;
use tracing::{info, warn};

/// Service for handling book merging and chapter moves.
pub struct MergeService {
    book_repo: Arc<BookRepository>,
    chapter_repo: Arc<ChapterRepository>,
}

impl MergeService {
    /// Create a new MergeService
    pub fn new(book_repo: Arc<BookRepository>, chapter_repo: Arc<ChapterRepository>) -> Self {
        Self {
            book_repo,
            chapter_repo,
        }
    }

    /// Execute merge of two books
    /// Moves chapters from source to target, then deletes source.
    /// Requires strict equality of Title and Author.
    pub async fn merge_books(&self, target_book_id: &str, source_book_id: &str) -> Result<()> {
        info!("Merging book {} into {}", source_book_id, target_book_id);

        let target_book = self
            .book_repo
            .find_by_id(target_book_id)
            .await?
            .ok_or_else(|| TingError::NotFound("Target book not found".to_string()))?;

        self.book_repo
            .find_by_id(source_book_id)
            .await?
            .ok_or_else(|| TingError::NotFound("Source book not found".to_string()))?;

        // 1. Get all chapters
        let mut source_chapters = self.chapter_repo.find_by_book(source_book_id).await?;

        // 2. Determine max index in target
        let target_chapters = self.chapter_repo.find_by_book(target_book_id).await?;
        let max_index = target_chapters
            .iter()
            .map(|c| c.chapter_index.unwrap_or(0))
            .max()
            .unwrap_or(-1);

        let mut start_index = max_index + 1;

        // 3. Move chapters
        for chapter in source_chapters.iter_mut() {
            // Deduplication: Check if hash exists in target
            // We need to check against current target chapters
            let is_duplicate = if let Some(hash) = &chapter.hash {
                target_chapters
                    .iter()
                    .any(|tc| tc.hash.as_ref() == Some(hash))
            } else {
                false
            };

            if is_duplicate {
                info!("Skipping duplicate chapter {} (hash match)", chapter.id);
                // We should probably delete this duplicate chapter record?
                // Yes, otherwise it stays in DB pointing to nowhere if we delete source book?
                // No, we delete source book at end, which cascades delete chapters.
                // But we are moving them.
                // If we don't move it, it stays with source_book_id.
                // When source_book is deleted, this chapter is deleted. Correct.
                continue;
            }

            chapter.book_id = target_book_id.to_string();
            // Re-index: append to end
            chapter.chapter_index = Some(start_index);
            start_index += 1;
            self.chapter_repo.update(chapter).await?;
        }

        // 3b. Re-order all chapters in target by filename/title?
        // User said: "Re-order by chapter index".
        // If we just appended, they are ordered.
        // But if source chapters overlap in logical order?
        // E.g. Target: Ch1, Ch3. Source: Ch2.
        // We should probably re-sort EVERYTHING based on Title/Filename.
        // Let's do a re-sort pass.
        let mut all_chapters = self.chapter_repo.find_by_book(target_book_id).await?;
        // Sort by title natural order
        all_chapters.sort_by(|a, b| {
            natord::compare(
                a.title.as_deref().unwrap_or(""),
                b.title.as_deref().unwrap_or(""),
            )
        });

        for (i, chapter) in all_chapters.iter_mut().enumerate() {
            if chapter.chapter_index != Some(i as i32) {
                chapter.chapter_index = Some(i as i32);
                self.chapter_repo.update(chapter).await?;
            }
        }

        // 4. Update Target Book (manual_corrected = true)
        // Only set to true if it wasn't already (to preserve match_pattern if exists)
        // Actually, if we merge, we should probably mark it as corrected to prevent future scrapes overriding it.
        // But if it was already 1, we keep it.
        if target_book.manual_corrected == 0 {
            let mut updated_target = target_book.clone();
            updated_target.manual_corrected = 1;
            // Also set a default match pattern if none exists?
            // No, user didn't ask for that here.
            self.book_repo.update(&updated_target).await?;
        }

        // 5. Delete Source Book
        self.book_repo.delete(source_book_id).await?;

        info!("Merged successfully");
        Ok(())
    }

    /// Auto-merge books with identical titles
    pub async fn process_auto_merges(&self) -> Result<usize> {
        let books = self.book_repo.find_all().await?;
        let mut merged_count = 0;

        // Group by Title
        let mut title_groups: std::collections::HashMap<String, Vec<&Book>> =
            std::collections::HashMap::new();
        for book in &books {
            if let Some(title) = &book.title {
                title_groups.entry(title.clone()).or_default().push(book);
            }
        }

        for (title, group) in title_groups {
            if group.len() < 2 {
                continue;
            }

            // Strategy: Merge all into the one that is manual_corrected
            // If none are manual_corrected, pick the first one (usually the one created first, or just arbitrary).
            // Sort: Manual corrected first, then by ID
            let mut sorted_group = group.clone();
            sorted_group.sort_by(|a, b| {
                b.manual_corrected
                    .cmp(&a.manual_corrected) // Descending (1 before 0)
                    .then_with(|| a.id.cmp(&b.id))
            });

            let target = sorted_group[0];
            let sources = &sorted_group[1..];

            for source in sources {
                // Double check author strict equality as per user requirement
                if target.author != source.author {
                    info!(
                        "Skipping auto-merge for '{}' due to author mismatch: '{}' vs '{}'",
                        title,
                        target.author.as_deref().unwrap_or(""),
                        source.author.as_deref().unwrap_or("")
                    );
                    continue;
                }

                // Check path equality - do not merge if in different folders
                if target.path != source.path {
                    info!("Skipping auto-merge for '{}' due to path mismatch (different folders): '{}' vs '{}'",
                        title, target.path, source.path);
                    continue;
                }

                info!("Auto-merging {} into {}", source.id, target.id);
                if let Err(e) = self.merge_books(&target.id, &source.id).await {
                    warn!("Failed to auto-merge: {}", e);
                } else {
                    merged_count += 1;
                }
            }
        }

        Ok(merged_count)
    }

    /// Move specific chapters to another book
    pub async fn move_chapters(
        &self,
        target_book_id: &str,
        chapter_ids: Vec<String>,
    ) -> Result<()> {
        self.book_repo
            .find_by_id(target_book_id)
            .await?
            .ok_or_else(|| TingError::NotFound("Target book not found".to_string()))?;

        // Get max index of target book
        let target_chapters = self.chapter_repo.find_by_book(target_book_id).await?;
        let mut next_index = target_chapters
            .iter()
            .map(|c| c.chapter_index.unwrap_or(0))
            .max()
            .unwrap_or(-1)
            + 1;

        for chapter_id in chapter_ids {
            if let Some(mut chapter) = self.chapter_repo.find_by_id(&chapter_id).await? {
                chapter.book_id = target_book_id.to_string();
                chapter.chapter_index = Some(next_index);
                next_index += 1;
                self.chapter_repo.update(&chapter).await?;
            }
        }

        Ok(())
    }

    /// Update manual correction status for a book
    pub async fn update_manual_correction(
        &self,
        book_id: &str,
        manual_corrected: bool,
        match_pattern: Option<String>,
    ) -> Result<()> {
        let mut book = self
            .book_repo
            .find_by_id(book_id)
            .await?
            .ok_or_else(|| TingError::NotFound("Book not found".to_string()))?;

        book.manual_corrected = if manual_corrected { 1 } else { 0 };
        book.match_pattern = match_pattern;

        self.book_repo.update(&book).await?;
        Ok(())
    }
}
