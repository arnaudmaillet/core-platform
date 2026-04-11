#[cfg(test)]
mod tests {
    use crate::application::use_cases::lifecycle::unsuspend::{UnsuspendCommand, UnsuspendUseCase};
    use crate::application::utils::TestFixture;
    use crate::domain::account::builders::AccountIdentityBuilder;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, Locale};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_unsuspend_account_success() {
        let f = TestFixture::new(UnsuspendUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // 1. Arrange : On crée un compte et on le suspend (Version passe à 2)
        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("temp@test.com").unwrap(),
            ExternalId::from_raw("ext_unsuspend"),
        )
        .build();

        identity
            .suspend("Suspicious activity".into())
            .unwrap();
        identity.pull_events(); // On vide les events du setup
        let version_suspended = identity.version();

        f.identity_repo().insert(identity);

        let cmd = UnsuspendCommand {
            account_id,
        };

        // 2. Act : On s'attend à recevoir l'Account réactivé
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La levée de suspension devrait réussir");
        let updated = result.unwrap();

        assert_eq!(*updated.state(), AccountState::Active);
        assert_eq!(
            updated.version(),
            version_suspended + 1,
            "La version doit être incrémentée"
        );

        // 4. Persistence réelle
        let saved = f.identity_repo().find_by_id(&account_id).expect("Should exist");
        assert_eq!(*saved.state(), AccountState::Active);

        // 5. Outbox
        assert_eq!(f.outbox_repo().count(), 1, "Un événement AccountEvent::UNSUSPENDED attendu");
        assert!(f.outbox_events().contains(&AccountEvent::UNSUSPENDED.to_string()));
    }

    #[tokio::test]
    async fn test_unsuspend_idempotency() {
        let f = TestFixture::new(UnsuspendUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        let account = AccountIdentityBuilder::restore(
            account_id,
            region,
            ExternalId::from_raw("ext"),
            Email::try_new("active@test.com").unwrap(),
            true,
            None,
            false,
            AccountState::Active,
            None,
            Locale::default(),
            1,
            chrono::Utc::now(),
            chrono::Utc::now(),
            None,
        );
        f.identity_repo().insert(account);

        let cmd = UnsuspendCommand {
            account_id,
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await.unwrap();

        assert_eq!(result.version(), 1);
        assert_eq!(f.outbox_repo().count(), 0, "Idempotence : aucun événement produit");
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(UnsuspendUseCase::new);
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");
        
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                wrong_region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            ).build(),
        );

        let cmd = UnsuspendCommand {
            account_id,
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
