use crate::{domain::utils::RetryConfig, errors::Result};
use async_trait::async_trait;

#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Le type de contexte requis (ex: AccountContext ou AccountAppContext)
    type Context;

    /// La commande à traiter (ex: RegisterCommand)
    type Command;

    /// Ce que le handler retourne en cas de succès
    type Output;

    /// La méthode d'exécution principale
    async fn handle(&self, ctx: &Self::Context, cmd: Self::Command) -> Result<Self::Output>;

    fn retry_config(&self) -> RetryConfig {
        RetryConfig::default()
    }
}
