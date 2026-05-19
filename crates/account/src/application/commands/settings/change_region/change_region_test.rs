// crates/account/src/application/commands/settings/change_region_tests.rs

#[cfg(test)]
mod tests {
    use crate::application::commands::settings::ChangeRegionCommand;
    use crate::application::context::AccountContext;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::types::AccountState;
    use crate::repositories::AccountRepository;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{Error, ErrorCode, Result, Versioned};
    use shared_kernel::types::{Region, SubId};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_region_success() -> Result<()> {
        let f = TestFixture::new();
        let old_id = f.account_id();
        let new_region = Region::try_new("US")?;
        let test_sub_id = SubId::try_new("google-oauth2|123456")?;

        // 1. Arrange
        let account = f
            .account_builder()?
            .with_account_id(old_id)
            .with_sub_id(test_sub_id.clone())
            .with_state(AccountState::ACTIVE)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeRegionCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(old_id, f.region(), version_snapshot),
            new_region,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert

        // 💡 ALIGNEMENT : Le compte d'origine est toujours là sur le shard source,
        // mais sa version a été incrémentée suite à l'enregistrement du changement (OCC).
        let current_account = f
            .account_repo()
            .find_by_id(old_id, None)
            .await?
            .expect("Le compte d'origine doit toujours exister");

        assert_eq!(
            current_account.account_id(),
            old_id,
            "L'ID n'a pas encore changé à ce stade"
        );
        assert_eq!(current_account.version(), version_snapshot + 1);

        // 💡 LE RIGUEUR : C'est ici qu'on valide la réussite fonctionnelle du pattern Outbox.
        // On s'assure qu'un unique événement a bien été persisté dans la transaction,
        // prêt à être consommé par le worker pour exécuter le vrai déménagement physique.
        f.assert_outbox(1, Some(AccountEvent::REGION_CHANGED));

        Ok(())
    }

    #[tokio::test]
    async fn test_change_region_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let current_region = f.region();
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeRegionCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            new_region: current_region,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas bouger si la région demandée est strictement identique"
            );
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_change_region_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        // Arrange cohérent
        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);
        f.idempotency_repo().seed(cmd_id);

        let cmd = ChangeRegionCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            new_region: Region::try_new("US")?,
        };

        let result = f
            .bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente et renvoyer Ok"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_change_region_restricted_account() -> Result<()> {
        let f = TestFixture::new();

        let account = f
            .account_builder()?
            .with_state(AccountState::BANNED)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeRegionCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            new_region: Region::try_new("US")?,
        };

        let result = f
            .bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(matches!(
            result,
            Err(Error {
                code: ErrorCode::Forbidden,
                ..
            })
        ));
        Ok(())
    }
}
