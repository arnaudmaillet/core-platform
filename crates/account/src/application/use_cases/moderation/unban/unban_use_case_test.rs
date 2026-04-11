#[cfg(test)]
mod tests {
    use crate::application::use_cases::moderation::unban::{UnbanCommand, UnbanUseCase};
    use crate::application::utils::TestFixture;
    use crate::domain::account::builders::AccountIdentityBuilder;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, Locale};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_unban_account_success() {
        let f = TestFixture::new(UnbanUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // 1. Arrange : On crée un compte et on le bannit (Version passe à 2)
        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("clean@test.com").unwrap(),
            ExternalId::from_raw("ext_000"),
        )
        .build();

        identity.ban("Past violation".into()).unwrap();
        identity.pull_events(); // On nettoie les events du setup
        let version_banned = identity.version();

        f.identity_repo().insert(identity);

        let cmd = UnbanCommand {
            account_id,
        };

        // 2. Act : On s'attend à recevoir l'Account réactivé
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok(), "Le débannissement devrait réussir");
        let updated = result.unwrap();

        assert_eq!(*updated.state(), AccountState::Active);
        assert_eq!(
            updated.version(),
            version_banned + 1,
            "La version doit être incrémentée"
        );

        // 4. Persistence réelle
       let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(*saved.state(), AccountState::Active);

        // 5. Outbox
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::UNBANNED attendu"
        );
        assert!(f.outbox_events().contains(&AccountEvent::UNBANNED.to_string()));
    }

    #[tokio::test]
    async fn test_unban_idempotency() {
        let f = TestFixture::new(UnbanUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        // --- ARRANGE ---
        // Important : On restaure en état ACTIVE (pas Pending)
        let account = AccountIdentityBuilder::restore(
            account_id,
            region,
            ExternalId::from_raw("ext"),
            Email::try_new("active@test.com").unwrap(),
            true,
            None,
            false,
            AccountState::Active, // État cible déjà atteint
            None,
            Locale::default(),
            1,
            chrono::Utc::now(),
            chrono::Utc::now(),
            None,
        );
        f.identity_repo().insert(account);

        let cmd = UnbanCommand {
            account_id,
        };

        // --- ACT ---
        let result = f.use_case().execute(&f.ctx(), cmd).await.unwrap();

        // --- ASSERT ---
        assert_eq!(*result.state(), AccountState::Active);
        assert_eq!(result.version(), 1);

        // Vérification DB
       let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.version(), 1);

        // Vérification Outbox
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Idempotence : aucun événement généré"
        );
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(UnbanUseCase::new);
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

        let cmd = UnbanCommand {
            account_id,
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
