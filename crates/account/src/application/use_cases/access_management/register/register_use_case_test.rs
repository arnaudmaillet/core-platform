// crates/account/src/application/access_management/register/register_use_case_test.rs

#[cfg(test)]
mod tests {
    use tokio;

    use shared_kernel::domain::value_objects::AccountId;
    use shared_kernel::errors::DomainError;

    use crate::application::use_cases::access_management::register::{RegisterCommand, RegisterUseCase};
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, IpAddr, Locale};

    #[tokio::test]
    async fn test_register_success() {
        // 1. Setup : La fixture instancie tout le nécessaire (repos, ctx, use_case)
        let f = TestFixture::new(RegisterUseCase::new);
        
        let email = Email::try_new("new-user@example.com").unwrap();
        let region = f.region();
        let ext_id = ExternalId::from_raw("auth0|12345");
        let ip = IpAddr::try_new("127.0.0.1").unwrap();

        let command = RegisterCommand {
            email: email.clone(),
            region: region.clone(),
            external_id: ext_id.clone(),
            locale: Locale::try_new("en-US").unwrap(),
            ip_addr: ip.clone(),
        };

        // 2. Act : On passe le ctx de la fixture
        let result = f.use_case().execute(f.ctx(), command).await;

        // 3. Assert
        assert!(result.is_ok(), "Le register devrait réussir");

        let all_events = f.outbox_events();
        let account = result.unwrap();
        let new_id = account.account_id();

        // Vérification de la création de l'AccountIdentity via le helper find_by_id
        let saved_identity = f.identity_repo().find_by_id(new_id).expect("Identity non sauvegardée");
        assert_eq!(saved_identity.email(), &email);
        assert_eq!(saved_identity.region_code(), &region);
        assert_eq!(saved_identity.external_id(), &ext_id);
        assert_eq!(saved_identity.state(), &AccountState::Active);

        // Vérification de la création de l'AccountMetadata
        let saved_metadata = f.metadata_repo().find_by_id(new_id).expect("Metadata non sauvegardée");
        assert_eq!(saved_metadata.last_ip_addr(), Some(&ip));

        // Vérification de la création de l'AccountSettings
        let saved_settings = f.settings_repo().find_by_id(new_id).expect("Settings non sauvegardées");
        assert_eq!(saved_settings.account_id(), new_id);

        // Vérification de l'outbox via le helper de la fixture
        assert_eq!(f.outbox_repo().count(), 1, "Un événement AccountEvent::REGISTERED attendu");
        assert!(f.outbox_events().contains(&AccountEvent::REGISTERED.to_string()));
    }

    #[tokio::test]
    async fn test_register_fails_if_external_id_already_exists() {
        let f = TestFixture::new(RegisterUseCase::new);
        let existing_ext_id = ExternalId::from_raw("duplicate_id");

        // 1. Arrange : On pré-enregistre un compte avec cet external_id
        f.identity_repo().insert(
            AccountIdentity::builder(
                AccountId::new(),
                f.region(),
                Email::try_new("existing@test.com").unwrap(),
                existing_ext_id.clone(),
            )
            .build(),
        );

        let command = RegisterCommand {
            email: Email::try_new("new@test.com").unwrap(),
            region: f.region(),
            external_id: existing_ext_id,
            locale: Locale::try_new("en-US").unwrap(),
            ip_addr: IpAddr::try_new("127.0.0.1").unwrap(),
        };

        // 2. Act
        let result = f.use_case().execute(f.ctx(), command).await;

        // 3. Assert
        assert!(result.is_err());
        match result.unwrap_err() {
            DomainError::AlreadyExists { field, .. } => {
                assert_eq!(field, "external_id");
            }
            _ => panic!("Devrait retourner une erreur AlreadyExists"),
        }
    }

    #[tokio::test]
    async fn test_register_atomic_rollback_on_outbox_failure() {
        let f = TestFixture::new(RegisterUseCase::new);
        
        // 1. Arrange : On force une erreur sur l'outbox pour simuler un échec de transaction
        f.outbox_repo().set_error(DomainError::Internal("DB Crash".into()));

        let command = RegisterCommand {
            email: Email::try_new("atomic@test.com").unwrap(),
            region: f.region(),
            external_id: ExternalId::from_raw("atomic_ext"),
            locale: Locale::try_new("en-US").unwrap(),
            ip_addr: IpAddr::try_new("127.0.0.1").unwrap(),
        };

        // 2. Act
        let result = f.use_case().execute(f.ctx(), command).await;

        // 3. Assert
        assert!(result.is_err());
        
        if let Err(DomainError::Internal(msg)) = result {
            assert_eq!(msg, "DB Crash");
        }
    }
}
