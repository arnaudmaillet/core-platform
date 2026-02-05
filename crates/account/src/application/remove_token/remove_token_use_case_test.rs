#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::AccountSettings;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, PushToken, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::remove_token::{RemovePushTokenCommand, RemovePushTokenUseCase};
    use crate::domain::repositories::AccountSettingsRepositoryStub;

    fn setup() -> (RemovePushTokenUseCase, Arc<AccountSettingsRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let settings_repo = Arc::new(AccountSettingsRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = RemovePushTokenUseCase::new(settings_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, settings_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_remove_push_token_success() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let token_to_add = PushToken::from_raw("token_456");
        let token_to_remove =  PushToken::from_raw("token_123");

        // Arrange : On crée des settings avec deux tokens
        let mut settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        settings.add_push_token(token_to_remove.clone()).unwrap();
        settings.add_push_token(token_to_add.clone()).unwrap();
        settings.pull_events(); // On vide les events d'ajout
        settings_repo.add_settings(settings);

        let cmd = RemovePushTokenCommand {
            account_id: account_id.clone(),
            region_code: region,
            token: token_to_remove.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = settings_repo.settings_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(!saved.push_tokens().contains(&token_to_remove));
        assert!(saved.push_tokens().contains(&token_to_add));
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_remove_push_token_idempotency() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let non_existent_token = PushToken::from_raw("non_existent_token");

        // Arrange : Settings sans le token en question
        let settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        settings_repo.add_settings(settings);

        let cmd = RemovePushTokenCommand {
            account_id,
            region_code: region,
            token: non_existent_token,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Aucun event si le token n'existait pas");
    }

    #[tokio::test]
    async fn test_remove_push_token_fails_on_region_mismatch() {
        let (use_case, settings_repo, _) = setup();
        let account_id = AccountId::new();
        let any_token = PushToken::from_raw("any_token");

        // Settings en EU
        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), RegionCode::from_raw("eu")).build());

        let cmd = RemovePushTokenCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // Mismatch
            token: any_token,
        };

        // ⚠️ N'oublie pas d'ajouter le check de région dans ton Use Case !
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}