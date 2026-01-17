use crate::errors::Result;

#[async_trait::async_trait]
pub trait QueryHandler<Q> {
    type Output;
    async fn handle(&self, query: Q) -> Result<Self::Output>;
}