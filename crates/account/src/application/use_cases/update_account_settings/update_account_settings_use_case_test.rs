#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::entities::{AccountSettings, AppearanceSettings, ThemeMode};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::update_account_settings::{UpdateAccountSettingsCommand, UpdateAccountSettingsUseCase};
    use crate::domain::repositories::AccountSettingsRepositoryStub;

    fn setup() -> (UpdateAccountSettingsUseCase, Arc<AccountSettingsRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let settings_repo = Arc::new(AccountSettingsRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = UpdateAccountSettingsUseCase::new(settings_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, settings_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_update_settings_success() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let initial_settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        settings_repo.add_settings(initial_settings);

        let new_appearance = AppearanceSettings { theme: ThemeMode::Dark, high_contrast: true };
        let cmd = UpdateAccountSettingsCommand {
            account_id: account_id.clone(),
            region_code: region,
            privacy: None,
            notifications: None,
            appearance: Some(new_appearance.clone()),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = settings_repo.settings_map.lock().unwrap().get(&account_id).cloned().unwrap();

        // On vérifie que le thème a bien changé
        assert_eq!(saved.appearance().theme, ThemeMode::Dark);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_update_settings_idempotency() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // ✅ Utilisation du BUILDER
        let mut settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        let current_appearance = settings.appearance().clone();

        // On s'assure que les événements de création sont purgés
        settings.pull_events();
        settings_repo.add_settings(settings);

        let cmd = UpdateAccountSettingsCommand {
            account_id,
            region_code: region,
            privacy: None,
            notifications: None,
            appearance: Some(current_appearance),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        // L'entité ne doit pas produire d'événement si les données sont identiques
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }
}