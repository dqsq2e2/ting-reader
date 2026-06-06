//! Database module
//!
//! This module provides database management functionality including:
//! - Database connection pool management
//! - Repository pattern implementations
//! - Database migrations
//! - Data models and schemas

pub mod manager;
pub mod migrations;
pub mod models;
pub mod repository;

pub use manager::DatabaseManager;
pub use models::{Book, Chapter, Series, SeriesBook, TaskRecord, User};
pub use repository::{
    BookRepository, ChapterRepository, Repository, SeriesRepository, TaskRepository, UserRepository,
};
