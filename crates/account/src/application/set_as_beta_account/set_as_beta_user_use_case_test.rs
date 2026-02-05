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
    use crate::application::set_as_beta_account::{SetAsBetaAccountCommand, SetAsBetaAccountUseCase};
    use crate::domain::repositories::AccountMetadataRepositoryStub;

    fn setup() -> (SetAsBetaAccountUseCase, Arc<AccountMetadataRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = SetAsBetaAccountUseCase::new(metadata_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, metadata_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_set_beta_status_to_true_success() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : Nouveau compte (beta_tester = false par défaut)
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        let cmd = SetAsBetaAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
            status: true,
            reason: "Early adopter program".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert!(saved.is_beta_tester());
        assert!(saved.moderation_notes().unwrap().contains("Beta tester mode enabled"));
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_set_beta_status_idempotency() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : Déjà beta tester
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.set_beta_status(true, "init".into());
        metadata.pull_events(); // Clear events
        metadata_repo.add_metadata(metadata);

        let cmd = SetAsBetaAccountCommand {
            account_id,
            region_code: region,
            status: true, // On essaie de remettre à true
            reason: "Double call".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        // L'idempotence métier empêche de générer un nouvel event
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_set_beta_status_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), RegionCode::from_raw("eu")).build());

        let cmd = SetAsBetaAccountCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // Mismatch
            status: true,
            reason: "Wrong region".into(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}