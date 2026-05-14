use crate::core::error::{Result, TingError};
use crate::db::models::Book;
use crate::db::repository::{BookRepository, Repository};
use crate::api::models::{CreateBookRequest, UpdateBookRequest};
use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;

/// Book service for managing book business logic
pub struct BookService {
    book_repo: Arc<BookRepository>,
}

impl BookService {
    pub fn new(book_repo: Arc<BookRepository>) -> Self {
        Self { book_repo }
    }

    pub async fn get_all_books(&self) -> Result<Vec<Book>> {
        self.book_repo.find_all().await
    }

    pub async fn get_books_by_library(&self, library_id: &str) -> Result<Vec<Book>> {
        if library_id.trim().is_empty() {
            return Err(TingError::ValidationError("Library ID cannot be empty".to_string()));
        }
        self.book_repo.find_by_library(library_id).await
    }

    pub async fn get_book_by_id(&self, id: &str) -> Result<Option<Book>> {
        if id.trim().is_empty() {
            return Err(TingError::ValidationError("Book ID cannot be empty".to_string()));
        }
        self.book_repo.find_by_id(id).await
    }

    pub async fn get_book_by_hash(&self, hash: &str) -> Result<Option<Book>> {
        if hash.trim().is_empty() {
            return Err(TingError::ValidationError("Book hash cannot be empty".to_string()));
        }
        self.book_repo.find_by_hash(hash).await
    }

    pub async fn create_book(&self, request: CreateBookRequest) -> Result<Book> {
        self.validate_create_request(&request)?;

        if let Some(existing) = self.book_repo.find_by_hash(&request.hash).await? {
            return Err(TingError::ValidationError(
                format!("Book with hash {} already exists with ID {}", request.hash, existing.id)
            ));
        }

        let book_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let book = Book {
            id: book_id.clone(),
            library_id: request.library_id,
            title: request.title,
            author: request.author,
            narrator: request.narrator,
            cover_url: request.cover_url,
            theme_color: request.theme_color,
            description: request.description,
            skip_intro: request.skip_intro,
            skip_outro: request.skip_outro,
            path: request.path,
            hash: request.hash,
            tags: request.tags,
            genre: None,
            year: None,
            created_at: now,
            manual_corrected: 0,
            match_pattern: None,
            chapter_regex: None,
        };

        self.book_repo.create(&book).await?;
        Ok(book)
    }

    pub async fn update_book(&self, id: &str, request: UpdateBookRequest) -> Result<Book> {
        if id.trim().is_empty() {
            return Err(TingError::ValidationError("Book ID cannot be empty".to_string()));
        }

        let mut book = self.book_repo.find_by_id(id).await?
            .ok_or_else(|| TingError::NotFound(format!("Book with ID {} not found", id)))?;

        if let Some(library_id) = request.library_id {
            if library_id.trim().is_empty() {
                return Err(TingError::ValidationError("Library ID cannot be empty".to_string()));
            }
            book.library_id = library_id;
        }
        if let Some(title) = request.title { book.title = Some(title); }
        if let Some(author) = request.author { book.author = Some(author); }
        if let Some(narrator) = request.narrator { book.narrator = Some(narrator); }

        if let Some(cover_url) = request.cover_url {
            let should_update = match &book.cover_url {
                Some(current) => current != &cover_url,
                None => true,
            };
            if should_update {
                book.cover_url = Some(cover_url.clone());
                if request.theme_color.is_none() {
                    use crate::core::color::calculate_theme_color;
                    tracing::info!("从封面 {} 重新计算书籍 {} 的主题颜色", book.id, cover_url);
                    match calculate_theme_color(&cover_url).await {
                        Ok(Some(color)) => {
                            tracing::info!("更新了书籍 {} 的主题颜色: {}", book.id, color);
                            book.theme_color = Some(color);
                        }
                        Ok(None) => {
                            tracing::warn!("无法从封面 {} 提取主题颜色", cover_url);
                            book.theme_color = None;
                        }
                        Err(e) => {
                            tracing::error!("计算主题颜色失败: {}", e);
                            book.theme_color = None;
                        }
                    }
                }
            }
        }
        if let Some(theme_color) = request.theme_color { book.theme_color = Some(theme_color); }
        if let Some(description) = request.description { book.description = Some(description); }
        if let Some(skip_intro) = request.skip_intro {
            if skip_intro < 0 { return Err(TingError::ValidationError("skip_intro cannot be negative".to_string())); }
            book.skip_intro = skip_intro;
        }
        if let Some(skip_outro) = request.skip_outro {
            if skip_outro < 0 { return Err(TingError::ValidationError("skip_outro cannot be negative".to_string())); }
            book.skip_outro = skip_outro;
        }
        if let Some(path) = request.path {
            if path.trim().is_empty() { return Err(TingError::ValidationError("Book path cannot be empty".to_string())); }
            book.path = path;
        }
        if let Some(hash) = request.hash {
            if hash.trim().is_empty() { return Err(TingError::ValidationError("Book hash cannot be empty".to_string())); }
            if let Some(existing) = self.book_repo.find_by_hash(&hash).await? {
                if existing.id != id {
                    return Err(TingError::ValidationError(
                        format!("Another book with hash {} already exists with ID {}", hash, existing.id)
                    ));
                }
            }
            book.hash = hash;
        }
        if let Some(tags) = request.tags { book.tags = Some(tags); }

        self.book_repo.update(&book).await?;
        Ok(book)
    }

    pub async fn delete_book(&self, id: &str) -> Result<()> {
        if id.trim().is_empty() {
            return Err(TingError::ValidationError("Book ID cannot be empty".to_string()));
        }
        let book = self.book_repo.find_by_id(id).await?
            .ok_or_else(|| TingError::NotFound(format!("Book with ID {} not found", id)))?;
        self.book_repo.delete(&book.id).await?;
        Ok(())
    }

    fn validate_create_request(&self, request: &CreateBookRequest) -> Result<()> {
        if request.library_id.trim().is_empty() {
            return Err(TingError::ValidationError("Library ID is required and cannot be empty".to_string()));
        }
        if request.path.trim().is_empty() {
            return Err(TingError::ValidationError("Book path is required and cannot be empty".to_string()));
        }
        if request.hash.trim().is_empty() {
            return Err(TingError::ValidationError("Book hash is required and cannot be empty".to_string()));
        }
        if request.skip_intro < 0 {
            return Err(TingError::ValidationError("skip_intro cannot be negative".to_string()));
        }
        if request.skip_outro < 0 {
            return Err(TingError::ValidationError("skip_outro cannot be negative".to_string()));
        }
        Ok(())
    }
}
