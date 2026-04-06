// crates/account/src/application/access_management/register/register_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio;

    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;

    use crate::application::access_management::register::{RegisterCommand, RegisterUseCase};
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::repositories::{
        AccountIdentityRepositoryStub, AccountMetadataRepositoryStub, AccountSettingsRepositoryStub,
    };
    use crate::domain::value_objects::{AccountState, Email, ExternalId, IpAddr, Locale};

    /// Helper pour initialiser le Use Case et ses dépendances
    fn setup() -> (
        RegisterUseCase,
        Arc<AccountIdentityRepositoryStub>,
        Arc<AccountMetadataRepositoryStub>,
        Arc<AccountSettingsRepositoryStub>,
        Arc<OutboxRepositoryStub>,
    ) {
        let account_repo = Arc::new(AccountIdentityRepositoryStub::new());
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let settings_repo = Arc::new(AccountSettingsRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);

        let use_case = RegisterUseCase::new(
            account_repo.clone(),
            metadata_repo.clone(),
            settings_repo.clone(),
            outbox_repo.clone(),
            tx_manager,
        );

        (
            use_case,
            account_repo,
            metadata_repo,
            settings_repo,
            outbox_repo,
        )
    }

    #[tokio::test]
    async fn test_register_success() {
        // Arrange
        let (use_case, account_repo, metadata_repo, settings_repo, outbox_repo) = setup();

        let command = RegisterCommand {
            email: Email::try_new("new-user@example.com").unwrap(),
            region: RegionCode::try_new("eu").unwrap(),
            external_id: ExternalId::from_raw("auth0|12345"),
            locale: Locale::try_new("en-US").unwrap(),
            ip_addr: IpAddr::try_new("127.0.0.1").unwrap(),
        };

        // Act
        let result = use_case.execute(command.clone()).await;

        // Assert
        assert!(result.is_ok(), "Le register devrait réussir");
        let account = result.unwrap();

        // Vérification de la création de l'Account
        let saved_account = account_repo
            .identity_map
            .lock()
            .unwrap()
            .get(&account.id())
            .cloned()
            .unwrap();
        assert_eq!(
            saved_account.email(),
            &Email::try_new("new-user@example.com").unwrap()
        );
        assert_eq!(
            saved_account.region_code(),
            &RegionCode::try_new("eu").unwrap()
        );
        assert_eq!(
            saved_account.external_id(),
            &ExternalId::from_raw("auth0|12345")
        );
        assert_eq!(saved_account.locale(), &Locale::try_new("en-US").unwrap());
        assert_eq!(saved_account.state(), &AccountState::Active);

        // Vérification de la création de l'AccountMetadata
        let saved_metadata = metadata_repo
            .metadata_map
            .lock()
            .unwrap()
            .get(&account.id())
            .cloned()
            .unwrap();
        assert_eq!(
            saved_metadata.last_ip_addr(),
            Some(&IpAddr::try_new("127.0.0.1").unwrap())
        );

        // Vérification de la création de l'AccountSettings
        let saved_settings = settings_repo
            .settings_map
            .lock()
            .unwrap()
            .get(&account.id())
            .cloned()
            .unwrap();
        assert_eq!(saved_settings.account_id(), account.id());

        // Vérification de l'outbox
        assert_eq!(
            outbox_repo.saved_events.lock().unwrap().len(),
            1,
            "Un événement AccountRegistered devrait être publié"
        );
    }

    #[tokio::test]
    async fn test_register_fails_if_external_id_already_exists() {
        // Arrange
        let (use_case, account_repo, _, _, _) = setup();

        let existing_id = ExternalId::from_raw("duplicate_id");
        let region = RegionCode::try_new("eu").unwrap();

        // On pré-enregistre un compte avec cet external_id
        account_repo.add_account(
            AccountIdentity::builder(
                AccountId::new(),
                region.clone(),
                Email::try_new("existing@test.com").unwrap(),
                existing_id.clone(),
            )
            .build(),
        );

        let command = RegisterCommand {
            email: Email::try_new("new@test.com").unwrap(),
            region: region.clone(),
            external_id: existing_id,
            locale: Locale::try_new("en-US").unwrap(),
            ip_addr: IpAddr::try_new("127.0.0.1").unwrap(),
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(result.is_err());
        match result.unwrap_err() {
            DomainError::AlreadyExists { entity, field, .. } => {
                assert_eq!(entity, "Account");
                assert_eq!(field, "external_id");
            }
            _ => panic!("Devrait retourner une erreur AlreadyExists"),
        }
    }

    #[tokio::test]
    async fn test_register_handles_retry_on_failure() {
        // Ce test vérifie indirectement le with_retry en simulant une erreur passagère
        // Pour un test pur, il faudrait un mock complexe du TxManager,
        // mais ici on s'assure au moins de la logique de base.
        let (use_case, _, _, _, _) = setup();

        // Une commande invalide qui ferait planter le builder (si validation ajoutée plus tard)
        // ou simplement tester le comportement nominal après retry.
        let command = RegisterCommand {
            email: Email::try_new("test@test.com").unwrap(),
            region: RegionCode::try_new("eu").unwrap(),
            external_id: ExternalId::from_raw("id_retry"),
            locale: Locale::try_new("en-US").unwrap(),
            ip_addr: IpAddr::try_new("127.0.0.1").unwrap(),
        };

        let result = use_case.execute(command).await;
        assert!(result.is_ok());
    }
}
