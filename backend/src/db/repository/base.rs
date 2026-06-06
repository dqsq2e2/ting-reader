use crate::core::error::Result;
use async_trait::async_trait;

/// Generic repository trait for CRUD operations
#[async_trait]
pub trait Repository<T>: Send + Sync {
    /// Find an entity by its ID
    async fn find_by_id(&self, id: &str) -> Result<Option<T>>;

    /// Find all entities
    async fn find_all(&self) -> Result<Vec<T>>;

    /// Create a new entity
    async fn create(&self, entity: &T) -> Result<()>;

    /// Update an existing entity
    async fn update(&self, entity: &T) -> Result<()>;

    /// Delete an entity by its ID
    async fn delete(&self, id: &str) -> Result<()>;
}
