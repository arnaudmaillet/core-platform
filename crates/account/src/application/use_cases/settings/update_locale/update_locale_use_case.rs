// crates/account/src/application/update_locale/update_locale_use_case.rs
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::settings::UpdateLocaleCommand;

pub struct UpdateLocaleHandler;

// crates/account/src/application/update_locale/update_locale_use_case.rs

#[async_trait]
impl CommandHandler for UpdateLocaleHandler {
    type Context = AccountContext;
    type Command = UpdateLocaleCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: UpdateLocaleCommand) -> Result<Self::Output> {
        // --- DEBUG 1 : Ce que le Use Case reçoit ---
        println!("DEBUG USECASE: Command AccountID cible: {}", cmd.account_id);

        // --- DEBUG 2 : Ce que le contexte contient ---
        // (Vérifie si ton AccountContext a une méthode pour logger son ID interne)
        // println!("DEBUG USECASE: Context Auth ID: {:?}", ctx.auth_id());

        let result = ctx.account().await;

        if let Err(ref e) = result {
            println!("DEBUG USECASE: ❌ ctx.account() a échoué: {:?}", e);
        }

        let mut account = result?;

        println!(
            "DEBUG USECASE: ✅ Compte chargé avec succès (Version: {})",
            account.version()
        );

        account.update_locale(cmd.new_locale)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}
