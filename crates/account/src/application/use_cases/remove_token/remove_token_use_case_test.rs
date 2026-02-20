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
    use crate::application::use_cases::remove_token::{RemovePushTokenCommand, RemovePushTokenUseCase};
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
        let region = RegionCode::try_new("eu").unwrap();
        let token_to_add = PushToken::try_new("valid_token_456").unwrap();
        let token_to_remove =  PushToken::try_new("valid_token_123").unwrap();

        // Arrange : On crée des settings avec deux tokens
        let mut settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        // On utilise la nouvelle signature avec région
        settings.add_push_token(&region, token_to_remove.clone()).unwrap();
        settings.add_push_token(&region, token_to_add.clone()).unwrap();
        settings.pull_events(); // On vide les events d'ajout
        settings_repo.add_settings(settings);

        let cmd = RemovePushTokenCommand {
            account_id: account_id.clone(),
            region_code: region.clone(),
            token: token_to_remove.clone(),
        };

        // Act : Doit renvoyer Ok(true)
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Ok(true)));
        let saved = settings_repo.settings_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(!saved.push_tokens().contains(&token_to_remove));
        assert!(saved.push_tokens().contains(&token_to_add));
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_remove_push_token_idempotency() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let non_existent_token = PushToken::try_new("valid_but_missing_token").unwrap();

        // Arrange : Settings vides
        let settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        settings_repo.add_settings(settings);

        let cmd = RemovePushTokenCommand {
            account_id,
            region_code: region,
            token: non_existent_token,
        };

        // Act : Doit renvoyer Ok(false) car rien n'a été supprimé
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Ok(false)));
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Idempotence : aucun event si le token n'existait pas");
    }

    #[tokio::test]
    async fn test_remove_push_token_fails_on_region_mismatch() {
        let (use_case, settings_repo, _) = setup();
        let account_id = AccountId::new();
        let any_token = PushToken::try_new("valid_token_abc_123").unwrap();

        // Settings en EU
        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), RegionCode::try_new("eu").unwrap()).build());

        let cmd = RemovePushTokenCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
            token: any_token,
        };

        let result = use_case.execute(cmd).await;

        // La protection de l'entité doit lever un Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}