#[cfg(test)]
mod tests {
    use crate::application::commands::access_management::LinkSubIdentityCommand;
    use crate::application::context::AccountContext;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use shared_kernel::core::{Error, ErrorCode, Result};
    use shared_kernel::types::SubId;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_link_sub_identity_success() -> Result<()> {
        let f = TestFixture::new();
        let account_id = f.account_id();
        let new_ext = SubId::try_new("google_123")?;

        // 1. Arrange : On utilise désormais None (Option)
        let account = f.account_builder_for(f.region())?.build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = LinkSubIdentityCommand {
            command_id: Uuid::new_v4(),
            account_id,
            sub_id: new_ext.clone(),
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, LinkSubIdentityCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            // CHANGEMENT : sub_id() renvoie &Option<SubId>
            // On compare donc avec Some(&new_ext) ou on as_ref()
            assert_eq!(acc.identity().sub_id(), Some(&new_ext));
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        // Vérifie que le nom de l'événement correspond bien à ta constante
        f.assert_outbox(1, Some(AccountEvent::EXTERNAL_LINKED));
        Ok(())
    }

    #[tokio::test]
    async fn test_link_sub_identity_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let ext_id = SubId::try_new("steam_456")?;

        // 1. Arrange: Le compte a déjà cet ID externe
        let mut account = f.account_builder()?.with_sub_id(ext_id.clone()).build()?;

        account.pull_events(); // On vide l'outbox de création
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = LinkSubIdentityCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id().clone(), f.region(), version_snapshot),
            sub_id: ext_id,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, LinkSubIdentityCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert: La version et l'outbox restent inchangées
        f.assert_account(|acc| {
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_link_sub_identity_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        f.idempotency_repo().seed(cmd_id);

        // On utilise un compte valide pour ne pas déclencher le "Forbidden"
        let account = f.account_builder()?.build()?;
        f.account_repo().insert(account);

        let cmd = LinkSubIdentityCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.account_id().clone(), f.region(), version_snapshot),
            // On met une valeur qui ne devrait pas poser de problème
            sub_id: SubId::try_new("apple_789")?,
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountContext, LinkSubIdentityCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // 3. Assert : Ici on s'attend à ce que l'infra bloque AVANT le domaine
        assert!(result.is_err(), "L'exécution aurait dû échouer");

        let err = result.unwrap_err();
        assert_eq!(
            err.code,
            ErrorCode::AlreadyExists,
            "Devrait échouer avec AlreadyExists (idempotence technique), reçu: {:?}",
            err
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_link_sub_identity_concurrency_retry() -> Result<()> {
        let f = TestFixture::new();
        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        // On prépare une erreur de concurrence unique
        f.account_repo()
            .set_error_once(Error::concurrency_conflict("Race condition"))?;

        let cmd = LinkSubIdentityCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id().clone(), f.region(), version_snapshot),
            sub_id: SubId::try_new("discord_000")?,
        };

        // 2. Act : Le CommandBus doit gérer le retry automatiquement
        f.bus()
            .execute::<AccountContext, LinkSubIdentityCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        Ok(())
    }
}
