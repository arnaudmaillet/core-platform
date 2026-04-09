#[cfg(test)]
mod tests {
    use crate::application::use_cases::moderation::shadowban::{ShadowbanCommand, ShadowbanUseCase};
    use crate::domain::account::entities::AccountMetadata;
    use crate::domain::repositories::AccountMetadataRepositoryStub;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use std::sync::Arc;

    fn setup() -> (
        ShadowbanUseCase,
        Arc<AccountMetadataRepositoryStub>,
        Arc<OutboxRepositoryStub>,
    ) {
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case =
            ShadowbanUseCase::new(metadata_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, metadata_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_shadowban_account_success() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();

        // 1. Arrange : Compte sain (Version 1)
        metadata_repo
            .add_metadata(AccountMetadata::builder(account_id.clone()).build());

        let cmd = ShadowbanCommand {
            account_id: account_id.clone(),
            reason: "Spam behavior detected".into(),
        };

        // 2. Act : On récupère l'entité mise à jour
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert!(updated.is_shadowbanned());
        assert!(
            updated
                .moderation_notes()
                .unwrap()
                .contains("Spam behavior detected")
        );
        assert_eq!(updated.version(), 2);

        // 4. Persistence
        let saved = metadata_repo
            .metadata_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
        assert!(saved.is_shadowbanned());

        // 5. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_shadowban_idempotency() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : Déjà banni (Version 2)
        let mut metadata = AccountMetadata::builder(account_id.clone()).build();
        metadata.shadowban("First ban".into()).unwrap();
        metadata.pull_events(); // Clear events
        let version_after_ban = metadata.version();

        metadata_repo.add_metadata(metadata);

        let cmd = ShadowbanCommand {
            account_id: account_id.clone(),
            reason: "Second report".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert!(returned.is_shadowbanned());
        assert_eq!(returned.version(), version_after_ban);

        // 4. Outbox
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 0);
    }
}
