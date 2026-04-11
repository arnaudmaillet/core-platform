#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::update_timezone::update_timezone_command::UpdateTimezoneCommand;
    use crate::application::use_cases::settings::update_timezone::update_timezone_use_case::UpdateTimezoneUseCase;
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::{AccountIdentity, AccountSettings};
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{RegionCode, Timezone};
    use shared_kernel::errors::DomainError;

    #[tokio::test]
    async fn test_update_timezone_success() {
        let f = TestFixture::new(UpdateTimezoneUseCase::new);
        let account_id = f.account_id();

        f.settings_repo().insert(
            AccountSettings::builder(account_id)
                .with_timezone(Timezone::from_raw("UTC"))
                .build(),
        );

        let new_tz = Timezone::from_raw("Europe/Paris");
        let cmd = UpdateTimezoneCommand {
            account_id: account_id,
            new_timezone: new_tz.clone(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(result.is_ok());
        let saved = f
            .settings_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.timezone(), &new_tz);
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::TIMEZONE_CHANGED attendu"
        );
        assert!(
            f.outbox_events()
                .contains(&AccountEvent::TIMEZONE_UPDATED.to_string())
        );
    }

    #[tokio::test]
    async fn test_update_timezone_idempotency() {
        let f = TestFixture::new(UpdateTimezoneUseCase::new);
        let account_id = f.account_id();
        let current_tz = Timezone::from_raw("Europe/London");

        let mut settings = AccountSettings::builder(account_id)
            .with_timezone(current_tz.clone())
            .build();
        settings.pull_events();
        f.settings_repo().insert(settings);

        let cmd = UpdateTimezoneCommand {
            account_id,
            new_timezone: current_tz,
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        assert!(result.is_ok());
        // Pas de changement -> Pas d'event -> Pas de save
        assert_eq!(f.outbox_repo().count(), 0, "Aucun evewnement attendu");
    }

    #[tokio::test]
    async fn test_update_timezone_business_rule_violation() {
        let f = TestFixture::new(UpdateTimezoneUseCase::new);
        let account_id = f.account_id();

        f.settings_repo()
            .insert(AccountSettings::builder(account_id).build());

        let cmd = UpdateTimezoneCommand {
            account_id,
            new_timezone: Timezone::from_raw("America/New_York"),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // On vérifie que le domaine rejette bien la timezone incohérente avec la région
        assert!(
            matches!(result, Err(DomainError::Validation { field, .. }) if field == "timezone")
        );
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(UpdateTimezoneUseCase::new);
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

        let cmd = UpdateTimezoneCommand {
            account_id,
            new_timezone: Timezone::from_raw("Europe/Paris"),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}
