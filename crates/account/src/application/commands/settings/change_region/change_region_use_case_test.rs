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
    use shared_kernel::types::{AccountId, RegionCode};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_region_success() -> Result<()> {
        let f = TestFixture::new();
        let old_id = f.account_id().clone();
        let new_region = RegionCode::from_raw("US");
        let expected_new_id = AccountId::new(old_id.uuid(), new_region.clone());

        // 1. Arrange
        let account = f
            .account_builder()?
            .with_account_id(old_id.clone()) // On s'assure qu'il part de l'ancien
            .with_state(AccountState::ACTIVE)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeRegionCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(old_id.clone(), f.region(), version_snapshot),
            new_region: new_region.clone(),
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        // On vérifie le nouvel ID
        f.assert_account_by_id(&expected_new_id, |acc| {
            assert_eq!(acc.account_id().region(), &new_region);
            assert_eq!(acc.account_id(), &expected_new_id);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        // Vérification de la suppression de l'ancien (Crucial pour éviter les fantômes)
        let old_exists = f.account_repo().find_by_id(&old_id, None).await?.is_some();
        assert!(!old_exists, "L'ancien ID (EU) devrait avoir été supprimé");

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
            target: CommandTarget::new(f.account_id().clone(), f.region(), version_snapshot),
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
                "La version ne doit pas bouger si la région est identique"
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
            target: CommandTarget::new(f.account_id().clone(), f.region(), version_snapshot),
            new_region: RegionCode::from_raw("US"),
        };

        let result = f
            .bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente (Ok)"
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
            target: CommandTarget::new(f.account_id().clone(), f.region(), version_snapshot),
            new_region: RegionCode::from_raw("US"),
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
