#[cfg(test)]
mod tests {
    use crate::application::context::AccountContext;
    use crate::application::use_cases::settings::ChangeRegionCommand;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::repositories::AccountRepository;
    use crate::domain::value_objects::AccountState;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_region_success() -> Result<()> {
        let f = TestFixture::new();
        let new_region = RegionCode::from_raw("us");

        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;
        let version_snapshot = account.version();
        // DEBUG pour confirmer
        assert_eq!(account.identity().region_code().as_str(), "eu");

        f.account_repo().insert(account.clone());

        let cmd = ChangeRegionCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_region: new_region.clone(),
        };

        // 1. On vérifie manuellement si le stub a l'objet
        let direct_check = f.account_repo().find_direct(&f.account_id());

        // 2. On vérifie via le trait (ce que le Bus utilise)
        let trait_check = f.account_repo().find_by_id(&f.account_id(), None).await?;

        f.bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        assert_eq!(
            account.identity().account_id().to_string(),
            f.account_id().to_string(),
            "L'ID de l'objet ne correspond pas à l'ID de la fixture !"
        );
        f.assert_account(|acc| {
            assert_eq!(acc.identity().region_code(), &new_region);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::REGION_CHANGED));
        Ok(())
    }

    #[tokio::test]
    async fn test_change_region_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let current_region = f.region();
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeRegionCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
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
        let account_id = f.account_id();

        // Arrange cohérent
        let account = f.account_builder()?.build()?;

        f.account_repo().insert(account);
        f.idempotency_repo().seed(cmd_id);

        let cmd = ChangeRegionCommand {
            command_id: cmd_id,
            account_id,
            new_region: RegionCode::from_raw("us"),
        };

        let result = f
            .bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(
            matches!(result, Err(DomainError::AlreadyExists { .. })),
            "Résultat inattendu: {:?}",
            result
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_change_region_restricted_account() -> Result<()> {
        let f = TestFixture::new();

        let account = f
            .account_builder()?
            .with_state(AccountState::Banned)
            .build()?;

        f.account_repo().insert(account);

        let cmd = ChangeRegionCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_region: RegionCode::from_raw("us"),
        };

        let result = f
            .bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let db_region = RegionCode::from_raw("us");
        let account = f.account_builder_for(db_region.clone())?.build()?;
        f.account_repo().insert(account);

        let cmd = ChangeRegionCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_region: RegionCode::from_raw("eu"),
        };

        let result = f
            .bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));
        Ok(())
    }
}
