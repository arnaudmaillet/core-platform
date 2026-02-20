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
    use crate::application::use_cases::add_push_token::{AddPushTokenCommand, AddPushTokenUseCase};
    use crate::domain::repositories::AccountSettingsRepositoryStub;

    // Helper pour initialiser le Use Case
    fn setup() -> (AddPushTokenUseCase, Arc<AccountSettingsRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let settings_repo = Arc::new(AccountSettingsRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
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
        let region = RegionCode::try_new("eu").unwrap();

        let settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        settings_repo.add_settings(settings);

        let token = PushToken::try_new("valid_push_token_long_enough").unwrap();
        let cmd = AddPushTokenCommand {
            account_id: account_id.clone(),
            region_code: region, // Ajout de la région dans la commande
            token: token.clone()
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());

        // Vérifier la persistance
        let saved = settings_repo.settings_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(saved.push_tokens().contains(&token));
        assert_eq!(saved.version(), 2);

        // Vérifier l'événement Outbox
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_idempotency_stops_execution_early() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("us").unwrap();
        let token = PushToken::try_new("already_existing_token_123").unwrap();

        let mut settings = AccountSettings::builder(account_id.clone(), region.clone()).build();
        // On simule que le token est déjà présent via l'entité directement
        settings.add_push_token(&region, token.clone()).unwrap();
        settings_repo.add_settings(settings);

        let cmd = AddPushTokenCommand {
            account_id,
            region_code: region,
            token
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());

        // Grace au retour `false` de l'entité, le Use Case ne doit pas avoir
        // persisté d'événements dans l'outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "L'idempotence aurait dû stopper le Use Case");
    }

    #[tokio::test]
    async fn test_cross_region_security_failure() {
        let (use_case, settings_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();
        let malicious_region = RegionCode::try_new("us").unwrap();

        // Le compte est en EU
        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), actual_region).build());

        // La commande prétend être en US
        let cmd = AddPushTokenCommand {
            account_id,
            region_code: malicious_region,
            token: PushToken::try_new("some_valid_token_12345").unwrap(),
        };

        let result = use_case.execute(cmd).await;

        // Doit retourner une erreur Forbidden via l'entité
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_worst_case_retry_exhaustion() {
        let (use_case, settings_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), region.clone()).build());

        *settings_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "Database high pressure".into(),
        });

        let cmd = AddPushTokenCommand {
            account_id,
            region_code: region,
            token: PushToken::try_new("retry_token_test_12345").unwrap(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }

    #[tokio::test]
    async fn test_transaction_failure_propagation() {
        let (use_case, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), region.clone()).build());

        let error_msg = "Kafka/Outbox DB Error";
        *outbox_repo.force_error.lock().unwrap() = Some(DomainError::Internal(error_msg.into()));

        let cmd = AddPushTokenCommand {
            account_id,
            region_code: region,
            token: PushToken::try_new("token_trigger_failure_123").unwrap(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_err());
        if let Err(DomainError::Internal(msg)) = result {
            assert_eq!(msg, error_msg);
        }
    }
}