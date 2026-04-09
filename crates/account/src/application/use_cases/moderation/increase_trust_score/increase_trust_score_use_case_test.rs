#[cfg(test)]
mod tests {
    use crate::application::use_cases::moderation::increase_trust_score::{
        IncreaseTrustScoreCommand, IncreaseTrustScoreUseCase,
    };
    use crate::domain::account::entities::AccountMetadata;
    use crate::domain::repositories::AccountMetadataRepositoryStub;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use std::sync::Arc;
    use uuid::Uuid;

    fn setup() -> (
        IncreaseTrustScoreUseCase,
        Arc<AccountMetadataRepositoryStub>,
        Arc<OutboxRepositoryStub>,
    ) {
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case =
            IncreaseTrustScoreUseCase::new(metadata_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, metadata_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_increase_trust_score_success() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();

        // 1. Arrange : Score par défaut (ex: 50)
        metadata_repo.add_metadata(
            AccountMetadata::builder(account_id.clone())
                .with_trust_score(50)
                .build(),
        );

        let cmd = IncreaseTrustScoreCommand {
            action_id: uuid::Uuid::now_v7(),
            account_id: account_id.clone(),
            amount: 20, // 50 + 20 = 70
            reason: "Email verified".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let updated = result.unwrap();

        // 3. Assert
        assert_eq!(updated.trust_score(), 70);
        assert_eq!(updated.version(), 2);

        let saved = metadata_repo
            .metadata_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
        assert_eq!(saved.trust_score(), 70);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_increase_trust_score_cap_at_one_hundred() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();

        // 1. Arrange : On monte manuellement à 90
        let mut metadata = AccountMetadata::builder(account_id.clone())
            .with_trust_score(50)
            .build();
        metadata
            .increase_trust_score(uuid::Uuid::now_v7(), 40, "bump".into())
            .unwrap();
        metadata_repo.add_metadata(metadata);

        let cmd = IncreaseTrustScoreCommand {
            action_id: uuid::Uuid::now_v7(),
            account_id: account_id.clone(),
            amount: 50, // 90 + 50 -> Doit saturer à 100
            reason: "High activity".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let updated = result.unwrap();

        // 3. Assert
        assert_eq!(
            updated.trust_score(),
            100,
            "Le score ne doit pas dépasser 100"
        );
        assert_eq!(
            updated.version(),
            3,
            "La version doit avoir augmenté (90 -> 100 est un changement)"
        );

        let saved = metadata_repo
            .metadata_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
        assert_eq!(saved.trust_score(), 100);
    }

    #[tokio::test]
    async fn test_increase_trust_score_idempotency_at_max() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();

        // 1. Arrange : Déjà au max (100)
        let mut metadata = AccountMetadata::builder(account_id.clone()).build();
        metadata
            .increase_trust_score(uuid::Uuid::now_v7(), 100, "max out".into())
            .unwrap();
        metadata.pull_events(); // On vide les événements du setup
        let version_at_max = metadata.version();

        metadata_repo.add_metadata(metadata);

        let cmd = IncreaseTrustScoreCommand {
            action_id: uuid::Uuid::now_v7(),
            account_id,
            amount: 10,
            reason: "Should do nothing".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let returned = result.unwrap();

        // 3. Assert
        assert_eq!(returned.trust_score(), 100);
        assert_eq!(
            returned.version(),
            version_at_max,
            "La version ne doit pas bouger"
        );

        // 4. Assert : Aucun événement produit
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(
            events.len(),
            0,
            "Pas d'événement produit quand on est déjà au max"
        );
    }
}
