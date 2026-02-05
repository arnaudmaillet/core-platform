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
    use crate::application::shadowban::{ShadowbanAccountCommand, ShadowbanAccountUseCase};
    use crate::domain::repositories::AccountMetadataRepositoryStub;

    fn setup() -> (ShadowbanAccountUseCase, Arc<AccountMetadataRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = ShadowbanAccountUseCase::new(metadata_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, metadata_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_shadowban_account_success() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : Compte sain avec score par défaut (50)
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        let cmd = ShadowbanAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
            reason: "Spam behavior detected".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(saved.is_shadowbanned());
        assert!(saved.moderation_notes().unwrap().contains("Shadowbanned: Spam behavior detected"));
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_shadowban_idempotency() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : Le compte est déjà shadowbanned
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.shadowban("First ban".into());
        metadata.pull_events(); // On vide les événements
        metadata_repo.add_metadata(metadata);

        let cmd = ShadowbanAccountCommand {
            account_id,
            region_code: region,
            reason: "Second report".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        // L'idempotence de l'entité empêche de créer un nouvel événement
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Pas d'event si déjà banni");
    }

    #[tokio::test]
    async fn test_shadowban_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();

        // Metadata en EU
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), RegionCode::from_raw("eu")).build());

        let cmd = ShadowbanAccountCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // Mauvaise région
            reason: "Moderation".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}