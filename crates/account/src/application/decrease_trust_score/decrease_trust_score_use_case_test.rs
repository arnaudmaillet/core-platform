#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use uuid::Uuid;
    use crate::domain::entities::AccountMetadata;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::decrease_trust_score::{DecreaseTrustScoreCommand, DecreaseTrustScoreUseCase};
    use crate::domain::repositories::AccountMetadataRepositoryStub;

    fn setup() -> (DecreaseTrustScoreUseCase, Arc<AccountMetadataRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = DecreaseTrustScoreUseCase::new(metadata_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, metadata_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_decrease_trust_score_success() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // Arrange: Score par défaut (supposons 100 pour cet exemple)
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 30,
            reason: "Suspicious activity".into(),
        };

        // Act: Doit renvoyer Ok(true)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(true)));

        // Assert
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.trust_score(), 20);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_decrease_trust_score_clamping_and_shadowban() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let metadata = AccountMetadata::builder(account_id.clone(), region.clone())
            .with_trust_score(20)
            .build();

        metadata_repo.add_metadata(metadata);

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 50, // 20 - 50 -> Clamp à 0 (ou valeur négative) et Shadowban
            reason: "Heavy violation".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Si ça échoue encore ici, vérifie si ton entité autorise
        // une baisse de score sur un compte "sain" qui aboutit à un ban.
        assert!(matches!(result, Ok(true)));

        // Assert
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(saved.trust_score() <= 0);
        assert!(saved.is_shadowbanned());

        let events = outbox_repo.saved_events.lock().unwrap();
        // Doit contenir TrustScoreDecreased ET Shadowbanned
        assert!(events.len() >= 2);
    }

    #[tokio::test]
    async fn test_decrease_trust_score_idempotency_at_minimum() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // Arrange: Score déjà au minimum (ex: 0)
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.decrease_trust_score(&region, Uuid::now_v7(), 200, "drain".into()).unwrap();
        metadata.pull_events();
        metadata_repo.add_metadata(metadata);

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id,
            region_code: region,
            amount: 10,
            reason: "Already at min".into(),
        };

        // Act: Doit renvoyer Ok(false) car le score ne peut plus descendre
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(false)));

        // Assert
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), actual_region).build());

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
            amount: 10,
            reason: "Test".into(),
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard: renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_conflict() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        *metadata_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "DB Busy".into(),
        });

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id,
            region_code: region,
            amount: 1,
            reason: "Test".into(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }
}