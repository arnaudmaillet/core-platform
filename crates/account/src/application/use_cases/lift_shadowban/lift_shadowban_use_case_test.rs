#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::entities::account::AccountMetadata;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::lift_shadowban::{LiftShadowbanCommand, LiftShadowbanUseCase};
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
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : On crée un compte et on le bannit manuellement
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.shadowban(&region, "Initial ban".into()).unwrap();
        metadata.pull_events(); // On vide les events du ban initial
        let version_after_ban = metadata.version(); // Devrait être 2
        
        metadata_repo.add_metadata(metadata);

        let cmd = LiftShadowbanCommand {
            account_id: account_id.clone(),
            region_code: region.clone(),
            reason: "Appeal accepted".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La levée du shadowban devrait réussir");
        let updated = result.unwrap();

        assert!(!updated.is_shadowbanned(), "Le flag shadowban doit être à false");
        assert!(updated.moderation_notes().unwrap().contains("Appeal accepted"));
        assert_eq!(updated.version(), version_after_ban + 1);

        // 4. Persistence
        let saved = metadata_repo.metadata_map.lock().unwrap()
            .get(&account_id)
            .cloned()
            .expect("Metadata devrait être en base");
        assert!(!saved.is_shadowbanned());
        
        // 5. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1, "Un événement ShadowbanLifted doit être produit");
    }

    #[tokio::test]
    async fn test_lift_shadowban_idempotency() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : Compte sain par défaut (Version 1)
        let initial_metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata_repo.add_metadata(initial_metadata);

        let cmd = LiftShadowbanCommand {
            account_id,
            region_code: region,
            reason: "Accidental click".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert!(!returned.is_shadowbanned());
        assert_eq!(returned.version(), 1, "La version ne doit pas augmenter si aucun changement");

        // 4. Outbox
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 0, "Aucun événement si le compte était déjà libre");
    }

    #[tokio::test]
    async fn test_lift_shadowban_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), actual_region).build());

        let cmd = LiftShadowbanCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
            reason: "Wrong region".into(),
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard : renvoie Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}