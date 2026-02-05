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
    use crate::application::upgrade_role::{UpgradeRoleCommand, UpgradeRoleUseCase};
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
        let region = RegionCode::from_raw("eu");

        // Arrange : Nouveau compte avec rôle User par défaut
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        let cmd = UpgradeRoleCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_role: AccountRole::Moderator,
            reason: "Joined the safety team".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.role(), AccountRole::Moderator);
        assert!(saved.moderation_notes().unwrap().contains("Joined the safety team"));
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_upgrade_role_idempotency() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        // Arrange : Déjà modérateur
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.upgrade_role(AccountRole::Moderator, "init".into()).unwrap();
        metadata.pull_events(); // Clear events
        metadata_repo.add_metadata(metadata);

        let cmd = UpgradeRoleCommand {
            account_id,
            region_code: region,
            new_role: AccountRole::Moderator,
            reason: "Duplicate promotion".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        // L'idempotence métier empêche la création d'un événement si le rôle est identique
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_upgrade_role_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();

        // Metadata en EU
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), RegionCode::from_raw("eu")).build());

        let cmd = UpgradeRoleCommand {
            account_id,
            region_code: RegionCode::from_raw("us"), // 👈 Mismatch
            new_role: AccountRole::Admin,
            reason: "Wrong region test".into(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "region_code"));
    }
}