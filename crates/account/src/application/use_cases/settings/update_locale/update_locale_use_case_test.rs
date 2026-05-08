#[cfg(test)]
mod tests {
    use crate::application::context::AccountContext;
    use crate::application::use_cases::settings::UpdateLocaleCommand;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, Locale};
    use shared_kernel::domain::entities::Versioned;
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_locale_success() -> Result<()> {
        let f = TestFixture::new();
        let old_locale = Locale::from_raw("fr");
        let new_locale = Locale::from_raw("en");

        // 1. Arrange : Compte actif avec une locale spécifique
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .with_locale(old_locale)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UpdateLocaleCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_locale: new_locale.clone(),
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, UpdateLocaleCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.identity().locale(), &new_locale);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::LOCALE_UPDATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_locale_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();
        let requested_locale = Locale::from_raw("it");

        // Arrange : Commande déjà vue par l'infra
        f.idempotency_repo().seed(cmd_id);

        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UpdateLocaleCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            new_locale: requested_locale.clone(),
        };

        // Act
        let result = f
            .bus()
            .execute::<AccountContext, UpdateLocaleCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));

        // Vérification intégrité : pas de changement
        f.assert_account(|acc| {
            assert_ne!(acc.identity().locale(), &requested_locale);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_update_locale_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let current_locale = Locale::from_raw("de");

        // 1. Arrange : Compte possédant déjà cette locale
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .with_locale(current_locale.clone())
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UpdateLocaleCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            new_locale: current_locale,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, UpdateLocaleCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas bouger"
            );
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }
}
