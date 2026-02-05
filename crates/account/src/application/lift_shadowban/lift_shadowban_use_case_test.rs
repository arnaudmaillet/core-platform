#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::AccountMetadata;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::lift_shadowban::{LiftShadowbanCommand, LiftShadowbanUseCase};
    use crate::domain::repositories::AccountMetadataRepositoryStub;

    fn setup() -> (LiftShadowbanUseCase, Arc<AccountMetadataRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = LiftShadowbanUseCase::new(metadata_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, metadata_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_lift_shadowban_success() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : On crée un compte déjà shadowbanned
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.shadowban("Initial ban".into());
        metadata.pull_events(); // On vide les events du ban
        metadata_repo.add_metadata(metadata);

        let cmd = LiftShadowbanCommand {
            account_id: account_id.clone(),
            region_code: region,
            reason: "Appeal accepted".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(!saved.is_shadowbanned());
        assert!(saved.moderation_notes().unwrap().contains("Appeal accepted"));
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_lift_shadowban_idempotency() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : Le compte n'est PAS shadowbanned
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        let cmd = LiftShadowbanCommand {
            account_id,
            region_code: region,
            reason: "Accidental click".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Aucun event si déjà libre");
    }

    #[tokio::test]
    async fn test_lift_shadowban_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), RegionCode::from_raw("eu")).build());

        let cmd = LiftShadowbanCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // Mismatch
            reason: "Wrong region".into(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}