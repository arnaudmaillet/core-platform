#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::account::entities::AccountMetadata;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::lifecycle::upgrade_role::{UpgradeRoleCommand, UpgradeRoleUseCase};
    use crate::domain::repositories::AccountMetadataRepositoryStub;
    use crate::domain::value_objects::AccountRole;

    fn setup() -> (UpgradeRoleUseCase, Arc<AccountMetadataRepositoryStub>, Arc<OutboxRepositoryStub>) {
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case = UpgradeRoleUseCase::new(metadata_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, metadata_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_upgrade_role_success() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();

        // 1. Arrange : Nouveau compte (Rôle User par défaut, Version 1)
        metadata_repo.add_metadata(
            AccountMetadata::builder(account_id.clone()).build()
        );

        let cmd = UpgradeRoleCommand {
            account_id: account_id.clone(),
            new_role: AccountRole::Moderator,
            reason: "Joined the safety team".into(),
        };

        // 2. Act : On récupère l'entité Metadata mise à jour
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "L'upgrade de rôle devrait réussir");
        let updated = result.unwrap();

        assert_eq!(updated.role(), AccountRole::Moderator);
        assert!(updated.moderation_notes().unwrap().contains("Joined the safety team"));
        assert_eq!(updated.version(), 2, "La version doit passer à 2");

        // 4. Persistence
        let saved = metadata_repo.metadata_map.lock().unwrap()
            .get(&account_id).cloned().unwrap();
        assert_eq!(saved.role(), AccountRole::Moderator);
        
        // 5. Outbox
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1, "Un événement RoleUpgraded attendu");
    }

    #[tokio::test]
    async fn test_upgrade_role_idempotency() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();

        // 1. Arrange : Déjà modérateur (Version passe à 2 lors du setup)
        let mut metadata = AccountMetadata::builder(account_id.clone()).build();
        metadata.upgrade_role(AccountRole::Moderator, "init".into()).unwrap();
        metadata.pull_events(); // Clear events du setup
        let version_after_setup = metadata.version();
        
        metadata_repo.add_metadata(metadata);

        let cmd = UpgradeRoleCommand {
            account_id: account_id.clone(),
            new_role: AccountRole::Moderator, // On redemande Moderator
            reason: "Duplicate promotion".into(),
        };

        // 2. Act
        let result = use_case.execute(cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert_eq!(returned.role(), AccountRole::Moderator);
        assert_eq!(returned.version(), version_after_setup, "La version ne doit pas augmenter");

        // 4. Outbox
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 0, "Aucun événement produit si le rôle est identique");
    }
}