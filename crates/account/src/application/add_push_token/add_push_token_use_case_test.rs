#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::AccountSettings;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, PushToken};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::add_push_token::{AddPushTokenCommand, AddPushTokenUseCase};
    use crate::domain::repositories::AccountSettingsRepositoryStub;

    // Helper pour initialiser le Use Case avec des dépendances contrôlées
    fn setup() -> (AddPushTokenUseCase, Arc<AccountSettingsRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let settings_repo = Arc::new(AccountSettingsRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new()); // Stub amélioré pour compter les events
        let tx_manager = Arc::new(StubTxManager);

        let use_case = AddPushTokenUseCase::new(
            settings_repo.clone(),
            outbox_repo.clone(),
            tx_manager,
        );

        (use_case, settings_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_success_path_full_flow() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let settings = AccountSettings::builder(account_id.clone(), RegionCode::from_raw("eu")).build();
        settings_repo.add_settings(settings);

        let token = PushToken::try_new("valid_push_token_long_enough").unwrap();
        let cmd = AddPushTokenCommand { account_id: account_id.clone(), token: token.clone() };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());

        // 1. Vérifier la persistance
        let saved = settings_repo.settings_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(saved.push_tokens().contains(&token));
        assert_eq!(saved.version(), 2);

        // 2. Vérifier l'événement Outbox
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 1, "Un événement aurait dû être persisté dans l'outbox");
    }

    #[tokio::test]
    async fn test_idempotency_avoids_db_write() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let token = PushToken::try_new("already_existing_token_123").unwrap();

        let mut settings = AccountSettings::builder(account_id.clone(), RegionCode::from_raw("us")).build();
        settings.add_push_token(token.clone()).unwrap(); // Version passe à 2
        settings_repo.add_settings(settings);

        let cmd = AddPushTokenCommand { account_id, token };
        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());

        // Si le token existe déjà, on ne doit pas avoir d'événements (donc pas de save)
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Idempotence: aucun event ne doit être généré");
    }

    #[tokio::test]
    async fn test_worst_case_retry_exhaustion() {
        let (use_case, settings_repo, _) = setup();
        let account_id = AccountId::new();
        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), RegionCode::from_raw("eu")).build());

        // On simule une erreur de concurrence PERMANENTE
        *settings_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "Database high pressure".into(),
        });

        let cmd = AddPushTokenCommand {
            account_id,
            token: PushToken::try_new("retry_token_test_12345").unwrap(),
        };

        let result = use_case.execute(cmd).await;

        // Le Use Case doit finir par abandonner et retourner l'erreur
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_transaction_failure_propagation() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let initial_settings = AccountSettings::builder(account_id.clone(), RegionCode::from_raw("eu")).build();
        settings_repo.add_settings(initial_settings);

        // 1. On force l'erreur sur l'Outbox (le "Worst Case" après le save du repo)
        let error_msg = "Kafka/Outbox DB Error";
        *outbox_repo.force_error.lock().unwrap() = Some(DomainError::Internal(error_msg.into()));

        let cmd = AddPushTokenCommand {
            account_id: account_id.clone(),
            token: PushToken::try_new("token_trigger_failure_123").unwrap(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert : On vérifie que l'erreur est bien remontée
        assert!(result.is_err());
        if let Err(DomainError::Internal(msg)) = result {
            assert_eq!(msg, error_msg);
        } else {
            panic!("Should have returned an Internal error");
        }

        // Note technique : On ne vérifie pas la version (v1 vs v2) ici.
        // Le Stub n'étant pas une vraie DB, il ne gère pas le rollback mémoire.
        // La garantie de non-persistance (rollback) est testée dans les tests d'intégration.
    }

    #[tokio::test]
    async fn test_validation_token_too_short() {
        // Ce test vérifie que le Use Case n'est même pas démarré si la commande est invalide
        // Mais ici, c'est le PushToken::try_new qui nous protège
        let token_result = PushToken::try_new("short");
        assert!(token_result.is_err());
    }
}