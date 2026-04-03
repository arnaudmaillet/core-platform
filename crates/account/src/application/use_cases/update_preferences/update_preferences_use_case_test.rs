#[cfg(test)]
mod tests {
    use crate::application::use_cases::update_preferences::{
        UpdatePreferencesCommand, UpdatePreferencesUseCase,
    };
    use crate::domain::account::entities::AccountSettings;
    use crate::domain::preferences::models::{AppearancePreferences, ThemeMode};
    use crate::domain::repositories::AccountSettingsRepositoryStub;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use std::sync::Arc;

    fn setup() -> (
        UpdatePreferencesUseCase,
        Arc<AccountSettingsRepositoryStub>,
        Arc<OutboxRepositoryStub>,
    ) {
        let settings_repo = Arc::new(AccountSettingsRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case =
            UpdatePreferencesUseCase::new(settings_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, settings_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_update_settings_success() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let initial_settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        settings_repo.add_settings(initial_settings);

        let new_appearance = AppearancePreferences::builder()
            .with_theme(ThemeMode::Dark)
            .with_high_contrast(true)
            .build();
        let cmd = UpdatePreferencesCommand {
            account_id: account_id.clone(),
            region_code: region,
            privacy: None,
            notifications: None,
            appearance: Some(new_appearance.clone()),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = settings_repo
            .settings_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();

        // On vérifie que le thème a bien changé
        assert_eq!(saved.preferences().appearance().theme(), ThemeMode::Dark);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_update_appearance_preferences_idempotency() {
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. On définit une config spécifique
        let initial_appearance = AppearancePreferences::builder()
            .with_theme(ThemeMode::System)
            .with_high_contrast(true)
            .build();
        // 2. On injecte cette config via le builder pour être SUR de l'état de départ
        let mut settings = AccountSettings::builder(account_id, region.clone())
            .with_appearance(initial_appearance.clone())
            .build();

        // 3. On purge les événements créés par le build/restore initial
        let _ = settings.pull_events();

        // 4. On tente de mettre à jour avec EXACTEMENT la même config
        let changed = settings
            .update_appearance_preferences(&region, initial_appearance)
            .unwrap();

        // 5. Assertions
        assert!(!changed, "Update with identical data should return false");
        assert_eq!(
            settings.pull_events().len(),
            0,
            "No events should be emitted for idempotent update"
        );
    }
}
