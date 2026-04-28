#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::update_preferences::{
        UpdatePreferencesCommand, UpdatePreferencesHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::preferences::models::{AppearancePreferences, ThemeMode};
    use crate::domain::value_objects::AccountState;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_preferences_success() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Compte actif
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let new_appearance = AppearancePreferences::builder()
            .with_theme(ThemeMode::Dark)
            .with_high_contrast(true)
            .build();

        let cmd = UpdatePreferencesCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            privacy: None,
            notifications: None,
            appearance: Some(new_appearance.clone()),
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, UpdatePreferencesHandler)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(
                acc.settings().preferences().appearance().theme(),
                ThemeMode::Dark
            );
            assert!(acc.settings().preferences().appearance().high_contrast());
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::APPEARANCE_PREFS_UPDATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_preferences_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        f.idempotency_repo().seed(cmd_id);

        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;
        f.account_repo().insert(account);

        let cmd = UpdatePreferencesCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            privacy: None,
            notifications: None,
            appearance: None,
        };

        // Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, UpdatePreferencesHandler)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_preferences_business_idempotency() -> Result<()> {
        let f = TestFixture::new();

        let initial_appearance = AppearancePreferences::builder()
            .with_theme(ThemeMode::System)
            .with_high_contrast(true)
            .build();

        // 1. Arrange : Compte possédant déjà ces préférences
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .settings(|s| s.with_appearance(initial_appearance.clone()))
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UpdatePreferencesCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            privacy: None,
            notifications: None,
            appearance: Some(initial_appearance),
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, UpdatePreferencesHandler)
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

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("us");

        // Arrange : Compte US vs contexte EU
        let account = f
            .account_builder_for(wrong_region)?
            .with_state(AccountState::Active)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UpdatePreferencesCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            privacy: None,
            notifications: None,
            appearance: None,
        };

        // Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, UpdatePreferencesHandler)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        // Vérification intégrité
        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert_eq!(saved.version(), version_snapshot);

        Ok(())
    }
}
