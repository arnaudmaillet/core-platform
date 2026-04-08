#[cfg(test)]
mod tests {
    use crate::application::settings::change_birth_date::{
        ChangeBirthDateCommand, ChangeBirthDateUseCase,
    };
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::repositories::AccountIdentityRepositoryStub;
    use crate::domain::value_objects::{BirthDate, Email, ExternalId};
    use chrono::{TimeZone, Utc};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::outbox_repository_stub::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
    use shared_kernel::errors::DomainError;
    use std::sync::Arc;

    fn setup() -> (
        ChangeBirthDateUseCase,
        Arc<AccountIdentityRepositoryStub>,
        Arc<OutboxRepositoryStub>,
    ) {
        let account_repo = Arc::new(AccountIdentityRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        let tx_manager = Arc::new(StubTxManager);
        let use_case =
            ChangeBirthDateUseCase::new(account_repo.clone(), outbox_repo.clone(), tx_manager);
        (use_case, account_repo, outbox_repo)
    }

    fn adult_birth_date() -> BirthDate {
        let date = Utc
            .with_ymd_and_hms(2000, 1, 1, 0, 0, 0)
            .unwrap()
            .date_naive();
        BirthDate::try_new(date).unwrap()
    }

    // --- CAS 1 : SUCCÈS (HAPPY PATH) ---
    #[tokio::test]
    async fn test_change_birth_date_success() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(
            AccountIdentity::builder(
                account_id.clone(),
                region.clone(),
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
            account_id: account_id.clone(),
            birth_date: new_date.clone(),
        };

        // 1. On vérifie que execute renvoie Ok(true)
        let result = use_case.execute(cmd).await;
        assert!(result.is_ok());
        let updated_account = result.unwrap();
        assert_eq!(updated_account.birth_date(), Some(&new_date));

        // 2. Vérifier la persistance
        let saved = account_repo
            .identity_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();

        assert_eq!(saved.birth_date(), Some(&new_date));
        assert_eq!(saved.version(), 2);
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 1);
    }

    // --- CAS 2 : ERREUR DE RÉGION (SÉCURITÉ SHARD) ---
    #[tokio::test]
    async fn test_change_birth_date_fails_on_region_mismatch() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();

        account_repo.add_account(
            AccountIdentity::builder(
                account_id.clone(),
                RegionCode::from_raw("eu"),
                Email::try_new("alex@test.com").unwrap(),
                ExternalId::from_raw("ext_1"),
            )
            .build(),
        );

        let cmd = ChangeBirthDateCommand {
            account_id,
            birth_date: adult_birth_date(),
        };

        let result = use_case.execute(cmd).await;

        // L'entité renvoie Forbidden en cas de mismatch de région
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    // --- CAS 3 : IDEMPOTENCE (AUCUN CHANGEMENT) ---
    #[tokio::test]
    async fn test_change_birth_date_idempotency() {
        let (use_case, account_repo, outbox_repo) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let date = BirthDate::try_new(
            Utc.with_ymd_and_hms(1990, 1, 1, 0, 0, 0)
                .unwrap()
                .date_naive(),
        )
        .unwrap();

        // 1. On crée le compte via le builder (Version 1)
        let mut account = AccountIdentity::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("alex@test.com").unwrap(),
            ExternalId::from_raw("ext_1"),
        )
        .build();

        // 2. On applique le changement initial (Version passe de 1 à 2)
        account.change_birth_date(date.clone()).unwrap();

        // On vide les événements générés par ce premier changement pour ne pas polluer le test
        account.pull_events();

        let version_after_setup = account.version(); // Ceci sera 2
        account_repo.add_account(account);

        let cmd = ChangeBirthDateCommand {
            account_id: account_id.clone(),
            birth_date: date,
        };

        // 3. Act : Le Use Case s'exécute
        let result = use_case.execute(cmd).await;
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
        let saved_in_db = account_repo
            .identity_map
            .lock()
            .unwrap()
            .get(&account_id)
            .cloned()
            .unwrap();
        assert_eq!(saved_in_db.version(), version_after_setup);

        // 6. Assert : Aucun nouvel événement
        let events = outbox_repo.saved_events.lock().unwrap();
        assert_eq!(events.len(), 0);
    }

    // --- CAS 4 : WORST CASE - COMPTE BLOQUÉ/BANNI ---
    #[tokio::test]
    async fn test_change_birth_date_forbidden_when_banned() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        let mut account = AccountIdentity::builder(
            account_id.clone(),
            region.clone(),
            Email::try_new("h@k.com").unwrap(),
            ExternalId::from_raw("ext"),
        )
        .build();

        account.ban("Abuse".into()).unwrap();
        account_repo.add_account(account);

        let cmd = ChangeBirthDateCommand {
            account_id,
            birth_date: adult_birth_date(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    // --- CAS 5 : WORST CASE - COMPTE INEXISTANT ---
    #[tokio::test]
    async fn test_change_birth_date_not_found() {
        let (use_case, _, _) = setup();
        let cmd = ChangeBirthDateCommand {
            account_id: AccountId::new(),
            birth_date: adult_birth_date(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(
            result,
            Err(DomainError::NotFound {
                entity: "Account",
                ..
            })
        ));
    }

    // --- CAS 6 : CONCURRENCE EXTRÊME ---
    #[tokio::test]
    async fn test_worst_case_concurrency_exhaustion() {
        let (use_case, account_repo, _) = setup();
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");

        account_repo.add_account(
            AccountIdentity::builder(
                account_id.clone(),
                region.clone(),
                Email::try_new("a@b.com").unwrap(),
                ExternalId::from_raw("ext"),
            )
            .build(),
        );

        *account_repo.error_to_return.lock().unwrap() = Some(DomainError::ConcurrencyConflict {
            reason: "Always failing".into(),
        });

        let cmd = ChangeBirthDateCommand {
            account_id,
            birth_date: adult_birth_date(),
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }
}
