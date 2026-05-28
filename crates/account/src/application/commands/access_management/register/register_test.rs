// crates/account/src/application/access_management/register/register_use_case_test.rs

#[cfg(test)]
mod tests {
    use shared_kernel::core::{AggregateMetadata, Error, ErrorCode, Result, Versioned};
    use shared_kernel::types::{Email, SubId};
    use uuid::Uuid;

    use crate::application::commands::access_management::RegisterCommand;
    use crate::application::context::AccountCommandContext;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::types::{AccountState, IpAddr, Locale, RegistrationIdentifier};

    #[tokio::test]
    async fn test_register_success() -> Result<()> {
        // 1. Setup
        let f = TestFixture::new();
        let email = Email::try_new("new-user@example.com")?;
        let ext_id = SubId::from_raw("keycloak|12345");
        let ip = IpAddr::try_new("127.0.0.1")?;

        // La source unique de vérité pour l'identité de ce test
        let expected_account_id = f.account_id();

        let cmd = RegisterCommand {
            command_id: Uuid::new_v4(),
            region: f.region(),
            account_id: expected_account_id,
            sub_id: Some(ext_id.clone()),
            identifier: RegistrationIdentifier::from_email(email.clone()),
            locale: Locale::try_new("en-US")?,
            ip_addr: ip.clone(),
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountCommandContext, RegisterCommand, ()>(f.command_ctx().clone(), cmd)
            .await;

        // 3. Assert
        assert!(
            result.is_ok(),
            "Le register devrait réussir : {:?}",
            result.err()
        );
        f.assert_account_exists(expected_account_id).await?;

        f.assert_account_by_id(expected_account_id, |acc| {
            assert_eq!(acc.identity().email(), Some(&email));
            assert_eq!(acc.identity().sub_id(), Some(&ext_id));
            assert_eq!(acc.identity().state(), &AccountState::ACTIVE);
            assert_eq!(acc.governance().last_ip_addr(), Some(&ip));

            // v1 car INITIAL_VERSION (0) + 1 (register call)
            assert_eq!(acc.version(), AggregateMetadata::INITIAL_VERSION + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::REGISTERED));

        Ok(())
    }

    #[tokio::test]
    async fn test_register_fails_if_sub_id_already_exists() -> Result<()> {
        let f = TestFixture::new();
        let existing_ext_id = SubId::from_raw("duplicate_id");

        // 1. Arrange : Un compte possède déjà ce sub_id
        let existing_acc = f
            .builder()?
            .with_sub_id(existing_ext_id.clone())
            .build()?;
        f.account_repo().insert(existing_acc);

        let cmd = RegisterCommand {
            command_id: Uuid::new_v4(),
            region: f.region(),
            account_id: f.account_id(),
            sub_id: Some(existing_ext_id),
            identifier: RegistrationIdentifier::try_from_email("new@test.com")?,
            locale: Locale::try_new("en-US")?,
            ip_addr: IpAddr::try_new("127.0.0.1")?,
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountCommandContext, RegisterCommand, ()>(f.command_ctx().clone(), cmd)
            .await;

        // 3. Assert
        match result {
            Err(e) => {
                assert_eq!(e.code, ErrorCode::AlreadyExists);
                assert!(e.message.contains("sub_id"));
            }
            _ => panic!("Devrait échouer car le sub_id existe déjà"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_register_atomic_rollback_on_outbox_failure() -> Result<()> {
        let f = TestFixture::new();
        let error_msg = "Outbox DB Crash";

        // 1. Arrange : On force une erreur d'infrastructure dans l'outbox stub
        f.outbox_repo().set_error(Error::internal(error_msg));

        let cmd = RegisterCommand {
            command_id: Uuid::new_v4(),
            region: f.region(),
            account_id: f.account_id(),
            sub_id: Some(SubId::from_raw("atomic_ext")),
            identifier: RegistrationIdentifier::try_from_email("atomic@test.com")?,
            locale: Locale::try_new("en-US")?,
            ip_addr: IpAddr::try_new("127.0.0.1")?,
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountCommandContext, RegisterCommand, ()>(f.command_ctx().clone(), cmd)
            .await;

        // 3. Assert
        match result {
            Err(e) => {
                assert_eq!(e.code, ErrorCode::InternalError);
                assert_eq!(e.message, "An internal server error occurred");
                assert_eq!(e.source(), Some(error_msg));
            }
            Ok(_) => panic!("Should have failed due to atomic transactional rollback"),
        }

        Ok(())
    }
}
