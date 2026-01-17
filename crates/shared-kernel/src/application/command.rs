use crate::errors::Result;

#[async_trait::async_trait]
pub trait CommandHandler<C> {
    type Output;
    async fn handle(&self, command: C) -> Result<Self::Output>;
}