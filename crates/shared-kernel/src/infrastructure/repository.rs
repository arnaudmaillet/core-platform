// crates/shared-kernel/src/persistence/repository.rs
use async_trait::async_trait;
use crate::errors::Result;
use crate::domain::Identifier;

#[async_trait]
pub trait BaseRepository<E, ID>: Send + Sync
where
    ID: Identifier
{
    async fn find_by_id(&self, id: &ID) -> Result<Option<E>>;
    async fn save(&self, entity: &E) -> Result<()>;
    async fn delete(&self, id: &ID) -> Result<()>;
}