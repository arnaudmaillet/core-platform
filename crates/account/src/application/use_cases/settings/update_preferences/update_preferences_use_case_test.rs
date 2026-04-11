#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::update_preferences::{
        UpdatePreferencesCommand, UpdatePreferencesUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::{AccountIdentity, AccountSettings};
    use crate::domain::events::AccountEvent;
    use crate::domain::preferences::models::{AppearancePreferences, ThemeMode};
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_update_preferences_success() {
        let f = TestFixture::new(UpdatePreferencesUseCase::new);
        let account_id = f.account_id();

        let initial_settings = AccountSettings::builder(account_id).build();
        f.settings_repo().insert(initial_settings);

        let new_appearance = AppearancePreferences::builder()
            .with_theme(ThemeMode::Dark)
            .with_high_contrast(true)
            .build();
        let cmd = UpdatePreferencesCommand {
            account_id,
            privacy: None,
            notifications: None,
            appearance: Some(new_appearance.clone()),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(result.is_ok());
        let saved = f
            .settings_repo()
            .find_by_id(&account_id)
            .expect("Should exist");

        // On vérifie que le thème a bien changé
        assert_eq!(saved.preferences().appearance().theme(), ThemeMode::Dark);
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::APPEARANCE_PREFS_UPDATED attendu"
        );
        assert!(
            f.outbox_events()
                .contains(&AccountEvent::APPEARANCE_PREFS_UPDATED.to_string())
        );
    }

    #[test]
    fn test_update_appearance_preferences_idempotency() {
        let f: TestFixture<UpdatePreferencesUseCase> =
            TestFixture::new(UpdatePreferencesUseCase::new);
        let account_id = f.account_id();

        // 1. On définit une config spécifique
        let initial_appearance = AppearancePreferences::builder()
            .with_theme(ThemeMode::System)
            .with_high_contrast(true)
            .build();
        // 2. On injecte cette config via le builder pour être SUR de l'état de départ
        let mut settings = AccountSettings::builder(account_id)
            .with_appearance(initial_appearance.clone())
            .build();

        // 3. On purge les événements créés par le build/restore initial
        let _ = settings.pull_events();

        // 4. On tente de mettre à jour avec EXACTEMENT la même config
        let changed = settings
            .update_appearance_preferences(initial_appearance)
            .unwrap();

        // 5. Assertions
        assert!(!changed, "Update with identical data should return false");
        assert_eq!(
            settings.pull_events().len(),
            0,
            "No events should be emitted for idempotent update"
        );
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(UpdatePreferencesUseCase::new);
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");

        // On simule une donnée en base qui appartient aux "us"
        // alors que notre contexte est "eu"
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                wrong_region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let new_appearance = AppearancePreferences::builder()
            .with_theme(ThemeMode::Dark)
            .with_high_contrast(true)
            .build();
        let cmd = UpdatePreferencesCommand {
            account_id,
            privacy: None,
            notifications: None,
            appearance: Some(new_appearance.clone()),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
