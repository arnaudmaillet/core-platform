// crates/account/src/application/use_cases/access_management/register/mod.rs
use async_trait::async_trait;

use shared_kernel::application::CommandHandler;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};

use crate::application::context::{AccountAppContext, AccountContext};
use crate::application::use_cases::access_management::register::RegisterCommand;
use crate::domain::account::entities::Account;

pub struct RegisterHandler;

#[async_trait]
impl CommandHandler for RegisterHandler {
    type Context = AccountAppContext;
    type Command = RegisterCommand;
    type Output = AccountId;

    async fn handle(
        &self,
        app_ctx: &AccountAppContext,
        cmd: RegisterCommand,
    ) -> Result<Self::Output> {
        // 1. Validation de non-existence (Invariants métier)
        // On vérifie l'external_id (Keycloak sub)
        if app_ctx
            .account_repo()
            .exists_by_external_id(&cmd.external_id)
            .await?
        {
            return Err(DomainError::AlreadyExists {
                entity: "Account",
                field: "external_id",
                value: cmd.external_id.to_string(),
            });
        }

        // 2. Création de l'identifiant unique de notre domaine
        let account_id = AccountId::new();

        // 3. Construction de l'agrégat via le builder
        // CHANGEMENT ICI : on utilise cmd.identifier directement
        let mut account = Account::builder(
            account_id.clone(),
            cmd.region.clone(),
            cmd.identifier
        )
        .with_external_id(cmd.external_id)
        .with_locale(cmd.locale)
        .build()?;

        // 4. Exécution de la logique de domaine
        // C'est ici que l'IP est enregistrée et que l'événement AccountRegistered est généré
        account.register(cmd.region.clone(), cmd.ip_addr)?;

        // 5. Sauvegarde atomique via le Scoped Context
        // Cela garantit : Transaction DB + Outbox Event + Idempotence (command_id)
        let scoped_ctx = AccountContext::new(app_ctx.clone(), account_id.clone(), cmd.region);

        scoped_ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(account_id)
    }
}
