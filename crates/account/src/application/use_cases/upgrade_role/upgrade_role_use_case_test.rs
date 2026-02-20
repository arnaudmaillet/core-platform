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
    use crate::application::use_cases::upgrade_role::{UpgradeRoleCommand, UpgradeRoleUseCase};
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
        let region = RegionCode::try_new("eu").unwrap();

        // Arrange : Nouveau compte avec rôle User par défaut
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());

        let cmd = UpgradeRoleCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_role: AccountRole::Moderator,
            reason: "Joined the safety team".into(),
        };

        // Act : Doit renvoyer Ok(true)
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Ok(true)));
        let saved = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        assert_eq!(saved.role(), AccountRole::Moderator);
        assert!(saved.moderation_notes().unwrap().contains("Joined the safety team"));
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_upgrade_role_idempotency() {
        let (use_case, metadata_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        // Arrange : Déjà modérateur
        let mut metadata = AccountMetadata::builder(account_id.clone(), region.clone()).build();
        metadata.upgrade_role(&region, AccountRole::Moderator, "init".into()).unwrap();
        metadata.pull_events();
        metadata_repo.add_metadata(metadata);

        let cmd = UpgradeRoleCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_role: AccountRole::Moderator,
            reason: "Duplicate promotion".into(),
        };

        // Act : Doit renvoyer Ok(false) car le rôle est déjà identique
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Ok(false)));
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_upgrade_role_fails_on_region_mismatch() {
        let (use_case, metadata_repo, _) = setup();
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();

        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), actual_region).build());

        let cmd = UpgradeRoleCommand {
            account_id,
            region_code: RegionCode::try_new("us").unwrap(), // Mismatch
            new_role: AccountRole::Admin,
            reason: "Wrong region test".into(),
        };

        let result = use_case.execute(cmd).await;

        // Sécurité Shard : renvoie Forbidden via ensure_region_match
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }
}