#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::use_cases::remove_token::{
        RemovePushTokenCommand, RemovePushTokenUseCase,
    };
    use crate::domain::account::entities::AccountSettings;
    use crate::domain::repositories::AccountSettingsRepositoryStub;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, PushToken, RegionCode};
    use shared_kernel::errors::DomainError;
    use std::sync::Arc;

    fn setup() -> (
        RemovePushTokenUseCase,
        Arc<AccountSettingsRepositoryStub>,
        Arc<OutboxRepositoryStub>,
    ) {
        let settings_repo = Arc::new(AccountSettingsRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case =
            RemovePushTokenUseCase::new(settings_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, settings_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_remove_push_token_success() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let token_to_keep = PushToken::try_new("token_keep_456").unwrap();
        let token_to_remove = PushToken::try_new("token_remove_123").unwrap();

        // 1. Arrange : On prépare des settings avec DEUX tokens
        let mut settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        settings
            .add_push_token(&region, token_to_remove.clone())
            .unwrap();
        settings
            .add_push_token(&region, token_to_keep.clone())
            .unwrap();
        settings.pull_events(); // On vide les events d'ajout initiaux
        let version_after_setup = settings.version(); // Devrait être 3 (Init + Add + Add)

        settings_repo.add_settings(settings);

        let cmd = RemovePushTokenCommand {
            account_id: account_id.clone(),
            region_code: region.clone(),
            token: token_to_remove.clone(),
        };

        // 2. Act : On s'attend à recevoir les AccountSettings mis à jour
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La suppression du token devrait réussir");
        let updated = result.unwrap();

        // Vérification de l'état mémoire
        assert!(
            !updated.push_tokens().contains(&token_to_remove),
            "Le token doit être supprimé"
        );
        assert!(
            updated.push_tokens().contains(&token_to_keep),
            "Le token à garder doit toujours être là"
        );
        assert_eq!(updated.version(), version_after_setup + 1);

        // 4. Persistence
        let saved = settings_repo
            .settings_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
        assert!(!saved.push_tokens().contains(&token_to_remove));

        // 5. Outbox
        assert_eq!(
            outbox_repo.saved_events.lock().unwrap().len(),
            1,
            "Un événement PushTokenRemoved attendu"
        );
    }

    #[tokio::test]
    async fn test_remove_push_token_idempotency() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let non_existent_token = PushToken::try_new("valid_but_missing_token").unwrap();

        // 1. Arrange : Settings avec une liste de tokens vide (Version 1)
        let settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        settings_repo.add_settings(settings);

        let cmd = RemovePushTokenCommand {
            account_id,
            region_code: region,
            token: non_existent_token,
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert_eq!(
            returned.version(),
            1,
            "La version ne doit pas augmenter pour une suppression inexistante"
        );
        assert!(returned.push_tokens().is_empty());

        // 4. Outbox
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 0, "Idempotence : aucun événement généré");
    }

    #[tokio::test]
    async fn test_remove_push_token_fails_on_region_mismatch() {
        let (use_case, settings_repo, _) = setup();
        let account_id = AccountId::new();
        let any_token = PushToken::try_new("valid_token_abc_123").unwrap();

        // Settings en EU
        settings_repo.add_settings(
            AccountSettings::builder(account_id.clone(), RegionCode::try_new("eu").unwrap())
                .build(),
        );

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
