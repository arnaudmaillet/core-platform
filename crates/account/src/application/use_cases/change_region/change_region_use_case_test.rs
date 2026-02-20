#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::domain::entities::{Account, AccountMetadata, AccountSettings};
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, Username, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::change_region::{ChangeRegionCommand, ChangeRegionUseCase};
    use crate::domain::repositories::{AccountMetadataRepositoryStub, AccountRepositoryStub, AccountSettingsRepositoryStub};

    fn setup() -> (
        ChangeRegionUseCase,
        Arc<AccountRepositoryStub>,
        Arc<AccountMetadataRepositoryStub>,
        Arc<AccountSettingsRepositoryStub>,
        Arc<OutboxRepositoryStub>
    ) {
        let account_repo = Arc::new(AccountRepositoryStub::new());
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let settings_repo = Arc::new(AccountSettingsRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);

        let use_case = ChangeRegionUseCase::new(
            account_repo.clone(),
            metadata_repo.clone(),
            settings_repo.clone(),
            outbox_repo.clone(),
            tx_manager,
        );

        (use_case, account_repo, metadata_repo, settings_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_change_region_success_flow() {
        // Arrange
        let (use_case, account_repo, metadata_repo, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let old_region = RegionCode::from_raw("eu");
        let new_region = RegionCode::from_raw("us");

        // Initialisation des 3 agrégats en "eu"
        account_repo.add_account(Account::builder(
            account_id.clone(), old_region.clone(),
            Username::try_new("user1").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), old_region.clone()).build());
        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), old_region.clone()).build());

        let cmd = ChangeRegionCommand {
            account_id: account_id.clone(),
            region_code: old_region,
            new_region: new_region.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());

        // Vérifier que les 3 agrégats ont migré
        let a = account_repo.accounts.lock().unwrap().get(&account_id).cloned().unwrap();
        let m = metadata_repo.metadata_map.lock().unwrap().get(&account_id).cloned().unwrap();
        let s = settings_repo.settings_map.lock().unwrap().get(&account_id).cloned().unwrap();

        assert_eq!(a.region_code(), &new_region);
        assert_eq!(m.region_code(), &new_region);
        assert_eq!(s.region_code(), &new_region);

        // Vérifier que des événements ont été produits (AccountRegionChanged + MetadataRegionChanged)
        assert!(outbox_repo.saved_events.lock().unwrap().len() >= 2);
    }

    #[tokio::test]
    async fn test_change_region_idempotency() {
        // Arrange : Déjà en région "us"
        let (use_case, account_repo, metadata_repo, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("us");

        account_repo.add_account(Account::builder(
            account_id.clone(), region.clone(),
            Username::try_new("user1").unwrap(), Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());
        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), region.clone()).build());

        let cmd = ChangeRegionCommand {
            account_id,
            region_code: region.clone(),
            new_region: region,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(outbox_repo.saved_events.lock().unwrap().len(), 0, "Aucun event si même région");
    }

    #[tokio::test]
    async fn test_worst_case_partial_failure_outbox() {
        // Arrange
        let (use_case, account_repo, metadata_repo, settings_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(Account::builder(account_id.clone(), region.clone(), Username::try_new("user_test").unwrap(), Email::try_new("e@e.com").unwrap(), ExternalId::from_raw("x")).build());
        metadata_repo.add_metadata(AccountMetadata::builder(account_id.clone(), region.clone()).build());
        settings_repo.add_settings(AccountSettings::builder(account_id.clone(), region.clone()).build());

        // Simulation d'une erreur fatale lors de l'écriture de l'outbox
        *outbox_repo.force_error.lock().unwrap() = Some(DomainError::Internal("Transaction fail".into()));

        let cmd = ChangeRegionCommand {
            account_id: account_id.clone(),
            region_code: region,
            new_region: RegionCode::from_raw("us"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_err());
        assert!(matches!(result, Err(DomainError::Internal(msg)) if msg == "Transaction fail"));
        // En prod, le TransactionManager ferait un rollback. Ici, on vérifie que l'erreur est propagée.
    }
}