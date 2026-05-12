#[cfg(test)]
mod tests {
    use crate::application::context::AccountContext;
    use crate::application::use_cases::settings::ChangeRegionCommand;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::AccountState;
    use shared_kernel::domain::entities::Versioned;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::core::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_region_success() -> Result<()> {
        let f = TestFixture::new();
        let old_id = f.account_id(); // ex: EU:uuid
        let new_region = RegionCode::from_raw("US");

        // L'ID attendu après le changement
        let expected_new_id =
            shared_kernel::domain::value_objects::AccountId::new(old_id.uuid(), new_region.clone());

        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account.clone());

        let cmd = ChangeRegionCommand {
            command_id: Uuid::new_v4(),
            account_id: old_id.clone(),
            new_region: new_region.clone(),
        };

        // Act
        f.bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // ASSERTION CORRIGÉE :
        // On ne peut plus utiliser f.assert_account car elle cherche l'ancien ID (EU:uuid)
        // On doit chercher le compte via son NOUVEL ID (US:uuid)
        f.assert_account_by_id(&expected_new_id, |acc| {
            assert_eq!(acc.identity().region_code(), &new_region);
            assert_eq!(acc.identity().account_id(), &expected_new_id);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        // On vérifie aussi que l'ancien ID n'existe plus (facultatif mais propre)
        assert!(
            f.account_repo().find_direct(&old_id).is_none(),
            "L'ancien ID devrait avoir disparu"
        );

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
            new_region: RegionCode::from_raw("US"),
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
            .with_state(AccountState::BANNED)
            .build()?;

        f.account_repo().insert(account);

        let cmd = ChangeRegionCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_region: RegionCode::from_raw("US"),
        };

        let result = f
            .bus()
            .execute::<AccountContext, ChangeRegionCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
        Ok(())
    }
}
