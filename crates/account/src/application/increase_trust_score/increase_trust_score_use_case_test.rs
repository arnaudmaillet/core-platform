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
        let region = RegionCode::from_raw("eu");

        // Initialisation au score neutre (50)
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        let cmd = IncreaseTrustScoreCommand {
            action_id: Uuid::new_v4(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 20, // 50 + 20 = 70
            reason: "Email verified".into(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.trust_score(), 70);
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_increase_trust_score_cap_at_one_hundred() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // On part d'un score déjà élevé (90)
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.increase_trust_score(Uuid::new_v4(), 40, "bump".into()); // 50 + 40 = 90
        metadata_repo.add_metadata(metadata);

        let cmd = IncreaseTrustScoreCommand {
            action_id: Uuid::new_v4(),
            account_id: account_id.clone(),
            region_code: region,
            amount: 50, // 90 + 50 devrait être plafonné à 100
            reason: "High activity".into(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.trust_score(), 100, "Le score ne doit pas dépasser 100");
    }

    #[tokio::test]
    async fn test_increase_trust_score_idempotency_at_max() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Déjà à 100
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.increase_trust_score(Uuid::new_v4(), 100, "max out".into());
        metadata.pull_events(); // Clear events
        metadata_repo.add_metadata(metadata);

        let cmd = IncreaseTrustScoreCommand {
            action_id: Uuid::new_v4(),
            account_id,
            region_code: region,
            amount: 10,
            reason: "Should do nothing".into(),
        };

        let result = use_case.execute(cmd).await;

        assert!(result.is_ok());
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Aucun event si le score est déjà au max");
    }

    #[tokio::test]
    async fn test_increase_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), RegionCode::from_raw("eu")).build());

        let cmd = IncreaseTrustScoreCommand {
            action_id: Uuid::new_v4(),
            account_id,
            region_code: RegionCode::from_raw("us"), // Mismatch
            amount: 10,
            reason: "Fraud check".into(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}