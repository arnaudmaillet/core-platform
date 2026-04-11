#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::change_birth_date::{
        ChangeBirthDateCommand, ChangeBirthDateUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;

    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{BirthDate, Email, ExternalId};
    use chrono::{TimeZone, Utc};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    fn adult_birth_date() -> BirthDate {
        let date = Utc
            .with_ymd_and_hms(2000, 1, 1, 0, 0, 0)
            .unwrap()
            .date_naive();
        BirthDate::try_new(date).unwrap()
    }

    #[tokio::test]
    async fn test_change_birth_date_success() {
        let f = TestFixture::new(ChangeBirthDateUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id,
                region,
                Email::try_new("alex@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let date_raw = Utc
            .with_ymd_and_hms(1995, 5, 15, 0, 0, 0)
            .unwrap()
            .date_naive();
        let new_date = BirthDate::try_new(date_raw).unwrap();

        let cmd = ChangeBirthDateCommand {
            account_id,
            birth_date: new_date.clone(),
        };

        // 1. On vérifie que execute renvoie Ok(true)
        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(result.is_ok());
        let updated_account = result.unwrap();
        assert_eq!(updated_account.birth_date(), Some(&new_date));

        // 2. Vérifier la persistance
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");

        assert_eq!(saved.birth_date(), Some(&new_date));
        assert_eq!(saved.version(), 2);
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::BIRTH_DATE_CHANGED attendu"
        );
        assert!(f.outbox_events().contains(&AccountEvent::BIRTH_DATE_CHANGED.to_string()));
    }

    #[tokio::test]
    async fn test_change_birth_date_idempotency() {
        let f = TestFixture::new(ChangeBirthDateUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let date = BirthDate::try_new(
            Utc.with_ymd_and_hms(1990, 1, 1, 0, 0, 0)
                .unwrap()
                .date_naive(),
        )
        .unwrap();

        // 1. On crée le compte via le builder (Version 1)
        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("alex@test.com").unwrap(),
            ExternalId::from_raw("ext_1"),
        )
        .build();

        // 2. On applique le changement initial (Version passe de 1 à 2)
        identity.change_birth_date(date.clone()).unwrap();

        // On vide les événements générés par ce premier changement pour ne pas polluer le test
        identity.pull_events();

        let version_after_setup = identity.version(); // Ceci sera 2
        f.identity_repo().insert(identity);

        let cmd = ChangeBirthDateCommand {
            account_id,
            birth_date: date,
        };

        // 3. Act : Le Use Case s'exécute
        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(result.is_ok());

        let returned_account = result.unwrap();

        // 4. Assert : L'idempotence doit GARDER la version du setup
        assert_eq!(returned_account.birth_date(), Some(&date));
        assert_eq!(
            returned_account.version(),
            version_after_setup,
            "La version ne doit pas avoir bougé par rapport au setup"
        );

        // 5. Assert : Rien en DB ne doit avoir changé
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.version(), version_after_setup);

        // 6. Assert : Aucun nouvel événement
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Aucun evewnement attendu"
        );
    }

    #[tokio::test]
    async fn test_change_birth_date_forbidden_when_banned() {
        let f = TestFixture::new(ChangeBirthDateUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("h@k.com").unwrap(),
            ExternalId::from_raw("ext"),
        )
        .build();

        identity.ban("Abuse".into()).unwrap();
        f.identity_repo().insert(identity);

        let cmd = ChangeBirthDateCommand {
            account_id,
            birth_date: adult_birth_date(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_change_birth_date_not_found() {
        let f = TestFixture::new(ChangeBirthDateUseCase::new);
        let account_id = f.account_id();
        let cmd = ChangeBirthDateCommand {
            account_id,
            birth_date: adult_birth_date(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(
            result,
            Err(DomainError::NotFound {
                entity: "AccountIdentity",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_worst_case_concurrency_exhaustion() {
        let f = TestFixture::new(ChangeBirthDateUseCase::new);
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
            reason: "Always failing".into(),
        });

        let cmd = ChangeBirthDateCommand {
            account_id,
            birth_date: adult_birth_date(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(ChangeBirthDateUseCase::new);
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
        let cmd = ChangeBirthDateCommand {
            account_id,
            birth_date: adult_birth_date(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
