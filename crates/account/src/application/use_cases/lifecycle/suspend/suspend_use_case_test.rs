#[cfg(test)]
mod tests {
    use crate::application::use_cases::lifecycle::suspend::{SuspendCommand, SuspendUseCase};
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_suspend_account_success() {
        let f = TestFixture::new(SuspendUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // 1. Arrange : Compte actif (Version 1)
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                f.region(),
                Email::try_new("check@test.com").unwrap(),
                ExternalId::from_raw("ext_789"),
            )
            .build(),
        );

        let cmd = SuspendCommand {
            account_id,
            reason: "Under investigation for fraud".into(),
        };

        // 2. Act : On s'attend à recevoir l'Account mis à jour
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "La suspension devrait réussir");
        let updated = result.unwrap();

        assert_eq!(*updated.state(), AccountState::Suspended);
        assert_eq!(updated.version(), 2, "La version doit être passée à 2");

        // 4. Persistence réelle
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(*saved.state(), AccountState::Suspended);

        // 5. Outbox
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::SUSPENDED attendu"
        );
        assert!(f.outbox_events().contains(&AccountEvent::SUSPENDED.to_string()));
    }

    #[tokio::test]
    async fn test_suspend_idempotency() {
        let f = TestFixture::new(SuspendUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // 1. Arrange : On crée et on suspend manuellement
        let mut account = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("p@b.com").unwrap(),
            ExternalId::from_raw("ext"),
        )
        .build();

        account.suspend("Original reason".into()).unwrap();
        account.pull_events();
        let version_at_suspension = account.version();

        f.identity_repo().insert(account);

        let cmd = SuspendCommand {
            account_id,
            reason: "Second call".into(),
        };

        // 2. Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let returned = result.unwrap();

        assert_eq!(*returned.state(), AccountState::Suspended);
        assert_eq!(
            returned.version(),
            version_at_suspension,
            "La version ne doit pas augmenter"
        );

        // 4. Outbox
        assert_eq!(f.outbox_repo().count(), 0, "Idempotence : aucun événement produit");
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(SuspendUseCase::new);
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");
        let reason = "some_reason";

        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                wrong_region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let cmd = SuspendCommand {
            account_id,
            reason: reason.to_string(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
