#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::update_locale::{
        UpdateLocaleCommand, UpdateLocaleUseCase,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{Email, ExternalId, Locale};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_update_locale_success() {
        let f = TestFixture::new(UpdateLocaleUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        let identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("john@example.com").unwrap(),
            ExternalId::from_raw("ext_123"),
        )
        .with_locale(Locale::from_raw("fr"))
        .build();

        f.identity_repo().insert(identity);

        let new_locale = Locale::from_raw("en");
        let cmd = UpdateLocaleCommand {
            account_id,
            locale: new_locale.clone(),
        };

        // Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // Assert
        assert!(result.is_ok());
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.locale(), &new_locale);
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::LOCALE_CHANGED attendu"
        );
        assert!(
            f.outbox_events()
                .contains(&AccountEvent::LOCALE_UPDATED.to_string())
        );
    }

    #[tokio::test]
    async fn test_update_locale_idempotency() {
        let f = TestFixture::new(UpdateLocaleUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let current_locale = Locale::from_raw("de");

        let mut identity = AccountIdentity::builder(
            account_id,
            region,
            Email::try_new("hans@test.de").unwrap(),
            ExternalId::from_raw("ext_456"),
        )
        .with_locale(current_locale.clone())
        .build();

        identity.pull_events(); // Nettoyage
        f.identity_repo().insert(identity);

        let cmd = UpdateLocaleCommand {
            account_id,
            locale: current_locale,
        };

        // Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // Assert
        assert!(result.is_ok());
        // L'entité détecte qu'il n'y a pas de changement -> pas d'event -> pas de save transactionnel
        assert_eq!(f.outbox_repo().count(), 0, "Aucun evewnement attendu");
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(UpdateLocaleUseCase::new);
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

        let cmd = UpdateLocaleCommand {
            account_id,
            locale: Locale::from_raw("us"),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
