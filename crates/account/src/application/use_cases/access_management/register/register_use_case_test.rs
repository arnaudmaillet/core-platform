// crates/account/src/application/access_management/register/register_use_case_test.rs

#[cfg(test)]
mod tests {
    use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
    use shared_kernel::errors::DomainError;
    use uuid::Uuid;

    use crate::application::use_cases::access_management::register::{
        RegisterCommand, RegisterHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{
        AccountState, Email, ExternalId, IpAddr, Locale, RegistrationIdentifier,
    };

    #[tokio::test]
    async fn test_register_success() -> shared_kernel::errors::Result<()> {
        // 1. Setup
        let f = TestFixture::new();
        let email = Email::try_new("new-user@example.com")?;
        let ext_id = ExternalId::from_raw("keycloak|12345");
        let ip = IpAddr::try_new("127.0.0.1")?;

        let command = RegisterCommand {
            command_id: Uuid::new_v4(),
            external_id: ext_id.clone(),
            identifier: RegistrationIdentifier::from_email(email.clone()),
            region: f.region(), // ex: "eu"
            locale: Locale::try_new("en-US")?,
            ip_addr: ip.clone(),
        };

        // 2. Act : On utilise le bus ou l'appel direct au handler
        // Note: Le contexte est le AppContext global ici
        let result = f.bus().execute(f.app_ctx(), command, RegisterHandler).await;

        // 3. Assert
        assert!(result.is_ok(), "Le register devrait réussir");
        let account_id = result.unwrap();

        // Vérification de l'agrégat complet via la Fixture
        f.assert_account_exists(&account_id).await?;

        f.assert_account_by_id(&account_id, |acc| {
            // Vérification Identity
            assert_eq!(acc.identity().email(), Some(&email));
            assert_eq!(acc.identity().external_id(), &ext_id);
            assert_eq!(acc.identity().state(), &AccountState::Active);

            // Vérification Governance (Metadata/IP)
            assert_eq!(acc.governance().last_ip_addr(), Some(&ip));

            // Vérification Version initiale (v1 car 0 + register)
            assert_eq!(
                acc.metadata().version(),
                AggregateMetadata::INITIAL_VERSION + 1
            );
        })
        .await?;

        // Vérification de l'Outbox (via helper de fixture)
        // On vérifie que l'événement "AccountRegistered" est présent
        f.assert_outbox_contains(AccountEvent::REGISTERED);

        Ok(())
    }

    #[tokio::test]
    async fn test_register_fails_if_external_id_already_exists() -> shared_kernel::errors::Result<()>
    {
        let f = TestFixture::new();
        let existing_ext_id = ExternalId::from_raw("duplicate_id");

        // 1. Arrange : On pré-enregistre un compte existant dans le repo
        let existing_acc = f
            .account_builder()?
            .with_external_id(existing_ext_id.clone())?
            .build()?;
        f.account_repo().insert(existing_acc);

        let command = RegisterCommand {
            command_id: Uuid::new_v4(),
            external_id: existing_ext_id,
            identifier: RegistrationIdentifier::try_from_email("new@test.com")?,
            region: f.region(),
            locale: Locale::try_new("en-US")?,
            ip_addr: IpAddr::try_new("127.0.0.1")?,
        };

        // 2. Act
        let result = f.bus().execute(f.app_ctx(), command, RegisterHandler).await;

        // 3. Assert
        assert!(result.is_err());
        if let Err(DomainError::AlreadyExists { field, .. }) = result {
            assert_eq!(field, "external_id");
        } else {
            panic!("Devrait retourner une erreur AlreadyExists sur external_id");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_register_atomic_rollback_on_outbox_failure() -> shared_kernel::errors::Result<()>
    {
        let f = TestFixture::new();

        // 1. Arrange : On force une erreur sur l'outbox
        let error_msg = "Outbox DB Crash";
        f.outbox_repo()
            .set_error(DomainError::Infrastructure(error_msg.into()));

        let command = RegisterCommand {
            command_id: Uuid::new_v4(),
            external_id: ExternalId::from_raw("atomic_ext"),
            identifier: RegistrationIdentifier::try_from_email("atomic@test.com")?,
            region: f.region(),
            locale: Locale::try_new("en-US")?,
            ip_addr: IpAddr::try_new("127.0.0.1")?,
        };

        // 2. Act
        let result = f.bus().execute(f.app_ctx(), command, RegisterHandler).await;

        // 3. Assert
        assert!(result.is_err());

        // On vérifie que l'erreur est bien propagée
        assert!(matches!(result, Err(DomainError::Infrastructure(m)) if m == error_msg));

        Ok(())
    }
}
