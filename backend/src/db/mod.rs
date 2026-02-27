//! Database module
//!
//! This module provides database management functionality including:
//! - Database connection pool management
//! - Repository pattern implementations
//! - Database migrations
//! - Data models and schemas

pub mod manager;
pub mod models;
pub mod repository;
pub mod migrations;

pub use manager::DatabaseManager;
pub use models::{Book, Chapter, PluginRecord, TaskRecord, User};
pub use repository::{
    Repository, BookRepository, ChapterRepository, PluginRepository, TaskRepository, UserRepository
};
