#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::change_email::{
        ChangeEmailCommand, ChangeEmailUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects:: RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_change_email_success() {
        let f = TestFixture::new(ChangeEmailUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let old_email = Email::try_new("old@test.com").unwrap();
        let new_email = Email::try_new("new@test.com").unwrap();


        // On prépare le compte existant
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                old_email.clone(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let cmd = ChangeEmailCommand {
            account_id: account_id,
            new_email: new_email.clone(),
        };

        // 1. Act : On récupère l'Account retourné par le Use Case
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // On vérifie que c'est un succès
        assert!(result.is_ok());
        let identity = result.unwrap();

        // 2. Assert : Vérifier l'objet retourné (Mémoire)
        assert_eq!(identity.email(), &new_email);
        assert!(!identity.is_email_verified());

        // 3. Assert : Vérifier la persistence (Mock Repo)
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");

        assert_eq!(saved.email(), &new_email);
        assert_eq!(saved.version(), 2);

        // 4. Assert : Vérifier l'Outbox (Événements)
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::EMAIL_CHANGED attendu"
        );
        assert!(f.outbox_events().contains(&AccountEvent::EMAIL_CHANGED.to_string()));
    }

    #[tokio::test]
    async fn test_change_email_idempotency() {
        let f = TestFixture::new(ChangeEmailUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let email = Email::try_new("same@test.com").unwrap();

        // On insère un compte avec la version 1
        let initial_account = AccountIdentity::builder(
            account_id,
            region,
            email.clone(),
            ExternalId::from_raw("ext_1"),
        )
        .build();

        f.identity_repo().insert(initial_account.clone());

        let cmd = ChangeEmailCommand {
            account_id,
            new_email: email.clone(),
        };

        // 1. Act : L'exécution doit réussir mais ne rien modifier
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(result.is_ok());
        let returned_identity = result.unwrap();

        // 2. Assert : L'objet retourné doit être identique à l'initial
        assert_eq!(returned_identity.email(), &email);
        assert_eq!(returned_identity.version(), 1);

        // 3. Assert : Rien ne doit avoir été persisté (pas d'appel à save inutile)
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.version(), 1);

        // 4. Assert : Crucial - Aucun événement ne doit être produit
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Aucun evewnement attendu"
        );
    }

    #[tokio::test]
    async fn test_change_email_forbidden_when_restricted() {
        let f = TestFixture::new(ChangeEmailUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext_1"),
        )
        .build();

        // Un banni ne change pas son email
        identity.ban("Violation".into()).unwrap();
        f.identity_repo().insert(identity);

        let cmd = ChangeEmailCommand {
            account_id,
            new_email: Email::try_new("new@b.com").unwrap(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_conflict() {
        let f = TestFixture::new(ChangeEmailUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("a@b.com").unwrap(),
                ExternalId::from_raw("ext"),
            )
            .build(),
        );
        

        f.identity_repo().set_error(DomainError::ConcurrencyConflict {
            reason: "Version mismatch".into(),
        });

        let cmd = ChangeEmailCommand {
            account_id,
            new_email: Email::try_new("b@c.com").unwrap(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(ChangeEmailUseCase::new);
        let account_id = f.account_id();
        let wrong_region = RegionCode::from_raw("us");

        // On simule une donnée en base qui appartient aux "us"
        // alors que notre contexte est "eu"
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                wrong_region,
                Email::try_new("hacker@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let cmd = ChangeEmailCommand {
            account_id,
            new_email: Email::try_new("new@test.com").unwrap(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
