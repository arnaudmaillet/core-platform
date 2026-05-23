#[cfg(test)]
mod tests {
    use crate::application::commands::settings::UpdatePreferencesCommand;
    use crate::application::context::AccountContext;
    use crate::application::utils::TestFixture;
    use crate::events::AccountEvent;
    use crate::types::{AccountState, AppearancePreferences, ThemeMode};
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{Result, Versioned};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_preferences_success() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Compte actif
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let new_appearance = AppearancePreferences::builder()
            .with_theme(ThemeMode::Dark)
            .with_high_contrast(true)
            .build();

        let cmd = UpdatePreferencesCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            privacy: None,
            notifications: None,
            appearance: Some(new_appearance.clone()),
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, UpdatePreferencesCommand, ()>(f.account_ctx().clone(), cmd)
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
            .with_state(AccountState::ACTIVE)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UpdatePreferencesCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            privacy: None,
            notifications: None,
            appearance: None,
        };

        let result = f
            .bus()
            .execute::<AccountContext, UpdatePreferencesCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente (Ok)"
        );
        f.assert_outbox(0, None);
        f.assert_account(|acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas avoir augmenté"
            );
        })
        .await?;
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
            .with_state(AccountState::ACTIVE)
            .settings(|s| s.with_appearance(initial_appearance.clone()))
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UpdatePreferencesCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            privacy: None,
            notifications: None,
            appearance: Some(initial_appearance),
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, UpdatePreferencesCommand, ()>(f.account_ctx().clone(), cmd)
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
