#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::AccountSettings;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Timezone};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::update_timezone::update_timezone_command::UpdateTimezoneCommand;
    use crate::application::update_timezone::update_timezone_use_case::UpdateAccountTimezoneUseCase;
    use crate::domain::repositories::AccountSettingsRepositoryStub;

    fn setup() -> (UpdateAccountTimezoneUseCase, Arc<AccountSettingsRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let settings_repo = Arc::new(AccountSettingsRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = UpdateAccountTimezoneUseCase::new(settings_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, settings_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_update_timezone_success() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // ✅ Utilisation du Builder
        settings_repo.add_settings(
            AccountSettings::builder(account_id.clone(), region.clone())
                .with_timezone(Timezone::from_raw("UTC"))
                .build()
        );

        let new_tz = Timezone::from_raw("Europe/Paris");
        let cmd = UpdateTimezoneCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_timezone: new_tz.clone(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = settings_repo.settings_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.timezone(), &new_tz);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_update_timezone_idempotency() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let current_tz = Timezone::from_raw("Europe/London");

        let mut settings = AccountSettings::builder(account_id.clone(), region.clone())
            .with_timezone(current_tz.clone())
            .build();
        settings.pull_events();
        settings_repo.add_settings(settings);

        let cmd = UpdateTimezoneCommand {
            account_id,
            region_code: region,
            new_timezone: current_tz,
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        // Pas de changement -> Pas d'event -> Pas de save
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_update_timezone_business_rule_violation() {
        let (use_case, settings_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu"); // Shard Europe

        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), region.clone()).build());

        let cmd = UpdateTimezoneCommand {
            account_id,
            region_code: region,
            new_timezone: Timezone::from_raw("America/New_York"), // 👈 Violation de règle métier
        };

        let result = use_case.execute(cmd).await;

        // On vérifie que le domaine rejette bien la timezone incohérente avec la région
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "timezone"));
    }

    #[tokio::test]
    async fn test_update_timezone_fails_on_region_mismatch() {
        let (use_case, settings_repo, _) = setup();
        let account_id = AccountId::new();

        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), RegionCode::from_raw("eu")).build());

        let cmd = UpdateTimezoneCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // 👈 Mauvais shard spécifié
            new_timezone: Timezone::from_raw("UTC"),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}