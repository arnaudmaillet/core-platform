#[cfg(test)]
mod tests {
    use crate::application::utils::TestFixture;
    use crate::domain::account::builders::AccountIdentityBuilder;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, Email, ExternalId, Locale, PhoneNumber};
    use chrono::Utc;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;
    use shared_kernel::domain::events::AggregateRoot;
    use crate::application::use_cases::settings::change_phone_number::change_phone_number_command::ChangePhoneNumberCommand;
    use crate::application::use_cases::settings::change_phone_number::change_phone_number_use_case::ChangePhoneNumberUseCase;

    #[tokio::test]
    async fn test_change_phone_number_success() {
        let f = TestFixture::new(ChangePhoneNumberUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let old_phone = PhoneNumber::try_new("+33612345678").unwrap();
        let new_phone = PhoneNumber::try_new("+33687654321").unwrap();

        // RESTORE : On simule un compte existant en v1 avec l'ancien téléphone
        let identity = AccountIdentityBuilder::restore(
            account_id, region, ExternalId::from_raw("ext_1"),
            Email::try_new("test@test.com").unwrap(), true, Some(old_phone), true,
            AccountState::Active, None, Locale::default(),
            1, chrono::Utc::now(), chrono::Utc::now(), None
        );
        f.identity_repo().insert(identity);

        let cmd = ChangePhoneNumberCommand {
            account_id: account_id,
            new_phone: new_phone.clone(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await.unwrap();

        // Assert : v1 + 1 changement = v2
        assert_eq!(result.phone_number(), Some(&new_phone));
        assert_eq!(result.version(), 2); 
        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::PHONE_NUMBER_CHANGED attendu"
        );
        assert!(f.outbox_events().contains(&AccountEvent::PHONE_NUMBER_CHANGED.to_string()));
    }

    #[tokio::test]
    async fn test_change_phone_idempotency() {
        let f = TestFixture::new(ChangePhoneNumberUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let phone = PhoneNumber::try_new("+33600000000").unwrap();

        // --- ARRANGE ---
        // On simule un compte qui a DEJÀ le téléphone en base, avec une version propre (1).
        // C'est ça que "restore" fait : il crée l'état final sans déclencher les mutations.
        let identity = AccountIdentityBuilder::restore(
            account_id, region, ExternalId::from_raw("ext"),
            Email::try_new("a@b.com").unwrap(), true, Some(phone.clone()), true,
            AccountState::Active, None, Locale::default(),
            1,
            Utc::now(), Utc::now(), None
        );
        f.identity_repo().insert(identity);

        let cmd = ChangePhoneNumberCommand { 
            account_id: account_id,  
            new_phone: phone.clone() 
        };

        // --- ACT ---
        let result = f.use_case().execute(&f.ctx(), cmd).await.unwrap();

        // --- ASSERT ---
        // 1. L'objet retourné n'a pas bougé (toujours version 1)
        assert_eq!(result.phone_number(), Some(&phone));
        assert_eq!(result.version(), 1); 

        // 2. Vérification DB (ton point 3) : RIEN n'a été écrasé en base
        let saved = f
            .identity_repo()
            .find_by_id(&account_id)
            .expect("Should exist");
        assert_eq!(saved.version(), 1);

        // 3. Vérification Outbox (ton point 4) : AUCUN événement produit
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Aucun evewnement attendu"
        );
    }

    #[tokio::test]
    async fn test_worst_case_outbox_failure_propagation() {
        let f = TestFixture::new(ChangePhoneNumberUseCase::new);
        let account_id = f.account_id();
        let region = f.region();
        let error_msg = "Kafka/Outbox DB Error";

        f.identity_repo().insert(AccountIdentity::builder(
            account_id, region,
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        f.outbox_repo().set_error(DomainError::Internal(error_msg.into()));

        let cmd = ChangePhoneNumberCommand {
            account_id,
            new_phone: PhoneNumber::try_new("+33611223344").unwrap(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::Internal(m)) if m == error_msg));         
    }


    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(ChangePhoneNumberUseCase::new);
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

        let cmd = ChangePhoneNumberCommand { 
            account_id: account_id,  
            new_phone: PhoneNumber::try_new("+33611223344").unwrap(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}