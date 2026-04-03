#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::account::entities::AccountMetadata;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::set_as_beta_account::{SetAsBetaAccountCommand, SetAsBetaAccountUseCase};
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
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : Nouveau compte (beta_tester = false par défaut, version 1)
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        let cmd = SetAsBetaAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
            status: true,
            reason: "Early adopter program".into(),
        };

        // 2. Act : On s'attend à recevoir l'entité mise à jour
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert!(updated.is_beta_tester());
        assert!(updated.moderation_notes().unwrap().contains("Early adopter program"));
        assert_eq!(updated.version(), 2);

        // 4. Persistence
        let saved = metadata_repo.metadata_map.lock().unwrap()
            .get(&account_id).cloned().unwrap();
        assert!(saved.is_beta_tester());
        
        // 5. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_set_beta_status_idempotency() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // 1. Arrange : On le passe beta manuellement (Version passe à 2)
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.set_beta_status(&region, true, "initial activation".into()).unwrap();
        metadata.pull_events(); // On vide les events de l'initialisation
        let version_after_setup = metadata.version();
        
        metadata_repo.add_metadata(metadata);

        let cmd = SetAsBetaAccountCommand {
            account_id: account_id.clone(),
            region_code: region,
            status: true, // On demande encore true
            reason: "Double call".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert!(returned.is_beta_tester());
        assert_eq!(returned.version(), version_after_setup);

        // 4. Outbox
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_set_beta_status_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), actual_region).build());

        let cmd = SetAsBetaAccountCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
            status: true,
            reason: "Wrong region".into(),
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard : Forbidden
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}