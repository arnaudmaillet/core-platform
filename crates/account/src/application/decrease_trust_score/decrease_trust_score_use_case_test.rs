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
        let region = RegionCode::from_raw("eu");

        // On part du défaut (50)
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::new_v4(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 30, // 50 - 30
            reason: "Suspicious activity".into(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();

        assert_eq!(saved.trust_score(), 20);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_decrease_trust_score_floor_at_zero() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // On initialise un score manuellement bas (ex: 10) pour tester le floor
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        // On simule une baisse pour arriver à 10 (50 - 40)
        metadata.decrease_trust_score(Uuid::new_v4(), 40, "init".into());
        metadata.pull_events();
        metadata_repo.add_metadata(metadata);

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::new_v4(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 50, // 10 - 50 -> doit bloquer à 0
            reason: "Heavy violation".into(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.trust_score(), 0);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 2); // 1 pour le score + 1 pour le shadowban auto !
    }

    #[tokio::test]
    async fn test_decrease_trust_score_idempotency_at_zero() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Score déjà à 0
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.decrease_trust_score(Uuid::new_v4(), 100, "drain".into());
        metadata.pull_events();
        metadata_repo.add_metadata(metadata);

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::new_v4(),
            account_id,
            region_code: region,
            amount: 10,
            reason: "Irrelevant since score is 0".into(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 0, "Aucun événement ne doit être émis si le score ne change pas");
    }

    #[tokio::test]
    async fn test_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), RegionCode::from_raw("eu")).build());

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::new_v4(),
            account_id,
            region_code: RegionCode::from_raw("us"), // Mismatch
            amount: 10,
            reason: "Test".into(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_conflict() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        // Simulation d'un conflit de version constant
        *metadata_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "DB Busy".into(),
        });

        let cmd = DecreaseTrustScoreCommand {
            action_id: Uuid::new_v4(),
            account_id,
            region_code: region,
            amount: 1,
            reason: "Test".into(),
        };

        let result = use_case.execute(cmd).await;
        // Doit échouer après épuisement des retries
        assert!(matches!(result, Err(DomainError::ConcurrencyConflict { .. })));
    }
}