#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::application::use_cases::settings::change_region::{ChangeRegionCommand, ChangeRegionUseCase};
    use crate::domain::repositories::{AccountIdentityRepositoryStub};

    fn setup() -> (
        ChangeRegionUseCase,
        Arc<AccountIdentityRepositoryStub>,
        Arc<OutboxRepositoryStub>
    ) {
        let account_repo = Arc::new(AccountIdentityRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);

        let use_case = ChangeRegionUseCase::new(
            account_repo.clone(),
            outbox_repo.clone(),
            tx_manager,
        );

        (use_case, account_repo, outbox_repo)
    }

    #[tokio::test]
    async fn test_change_region_success_flow() {
        // Arrange
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let old_region = RegionCode::from_raw("eu");
        let new_region = RegionCode::from_raw("us");

        // Initialisation des 3 agrégats en "eu"
        account_repo.insert(AccountIdentity::builder(
            account_id.clone(), old_region.clone(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = ChangeRegionCommand {
            account_id: account_id.clone(),
            new_region: new_region.clone(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let response = result.unwrap();

        // Vérification de l'objet RETOURNÉ (Mémoire)
        assert_eq!(response.region_code().as_str(), "us");

        // Vérification de l'objet SAUVEGARDÉ (Persistence)
        let a = account_repo.identity_map.lock().unwrap().get(&account_id).cloned().unwrap();

        assert_eq!(a.region_code().as_str(), "us");
        
        // Vérification des versions (elles doivent toutes être à 2)
        assert_eq!(a.version(), 2);
    }

    #[tokio::test]
    async fn test_change_region_idempotency() {
        // Arrange : Déjà en région "us"
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("us");

        account_repo.insert(AccountIdentity::builder(
            account_id.clone(), region.clone(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = ChangeRegionCommand {
            account_id,
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
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.insert(AccountIdentity::builder(account_id.clone(), region.clone(), Email::try_new("e@e.com").unwrap(), ExternalId::from_raw("x")).build());

        // Simulation d'une erreur fatale lors de l'écriture de l'outbox
        *outbox_repo.force_error.lock().unwrap() = Some(DomainError::Internal("Transaction fail".into()));

        let cmd = ChangeRegionCommand {
            account_id: account_id.clone(),
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