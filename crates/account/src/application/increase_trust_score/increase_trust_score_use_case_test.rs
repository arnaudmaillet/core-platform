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
    use crate::application::increase_trust_score::{IncreaseTrustScoreCommand, IncreaseTrustScoreUseCase};
    use crate::domain::repositories::AccountMetadataRepositoryStub;

    fn setup() -> (IncreaseTrustScoreUseCase, Arc<AccountMetadataRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = IncreaseTrustScoreUseCase::new(metadata_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, metadata_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_increase_trust_score_success() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // Initialisation (score par défaut à 50)
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        let cmd = IncreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 20, // 50 + 20 = 70
            reason: "Email verified".into(),
        };

        // Act: doit renvoyer Ok(true)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(true)));

        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.trust_score(), 70);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_increase_trust_score_cap_at_one_hundred() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // On part d'un score déjà élevé (90)
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.increase_trust_score(&region, Uuid::now_v7(), 40, "bump".into()).unwrap();
        metadata_repo.add_metadata(metadata);

        let cmd = IncreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 50, // 90 + 50 -> Clamp à 100
            reason: "High activity".into(),
        };

        // Act: Le score change (90 -> 100), donc Ok(true)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(true)));

        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.trust_score(), 100);
    }

    #[tokio::test]
    async fn test_increase_trust_score_idempotency_at_max() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // Déjà au max (100)
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.increase_trust_score(&region, Uuid::now_v7(), 100, "max out".into()).unwrap();
        metadata.pull_events();
        metadata_repo.add_metadata(metadata);

        let cmd = IncreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id,
            region_code: region,
            amount: 10,
            reason: "Should do nothing".into(),
        };

        // Act: Le score ne peut pas bouger, donc Ok(false)
        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Ok(false)));

        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_increase_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), actual_region).build());

        let cmd = IncreaseTrustScoreCommand {
            action_id: Uuid::now_v7(),
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
            amount: 10,
            reason: "Fraud check".into(),
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard: renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}