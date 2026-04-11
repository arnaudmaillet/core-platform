#[cfg(test)]
mod tests {
    use crate::application::utils::TestFixture;
    use crate::domain::account::entities::AccountIdentity;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{Email, ExternalId};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::DomainError;
    use crate::application::use_cases::settings::change_region::{ChangeRegionCommand, ChangeRegionUseCase};

    #[tokio::test]
    async fn test_change_region_success_flow() {
        // 1. Arrange
        let f = TestFixture::new(ChangeRegionUseCase::new);
        let account_id = f.account_id();
        let region = f.region();

        let new_region = RegionCode::from_raw("us");

        // Initialisation de l'agrégat via le helper insert()
        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id, 
                region,
                Email::try_new("a@b.com").unwrap(),
                ExternalId::from_raw("ext")
            ).build()
        );

        let cmd = ChangeRegionCommand {
            account_id,
            new_region: new_region.clone(),
        };

        // 2. Act
        let result = f.use_case().execute(f.ctx(), cmd).await;

        // 3. Assert
        assert!(result.is_ok());
        let response = result.unwrap();

        // Vérification de l'objet RETOURNÉ (Mémoire)
        assert_eq!(response.region_code(), &new_region);

        // 4. Vérification de l'objet SAUVEGARDÉ (Persistence)
        let saved = f.identity_repo().find_by_id(&account_id).expect("Should exist");

        assert_eq!(saved.region_code(), &new_region);
        
        // Vérification de la version (elle doit être à 2 car sauvegardée une fois)
        assert_eq!(saved.version(), 2);

        assert_eq!(
            f.outbox_repo().count(),
            1,
            "Un événement AccountEvent::REGION_CHANGED attendu"
        );
        assert!(f.outbox_events().contains(&AccountEvent::REGION_CHANGED.to_string()));
    }

    #[tokio::test]
    async fn test_change_region_idempotency() {
        // Arrange : Déjà en région "us"
        let f = TestFixture::new(ChangeRegionUseCase::new);
        let account_id = f.account_id();

        f.identity_repo().insert(AccountIdentity::builder(
            account_id, f.region(),
            Email::try_new("a@b.com").unwrap(),
            ExternalId::from_raw("ext")
        ).build());

        let cmd = ChangeRegionCommand {
            account_id,
            new_region: f.region(),
        };

        // Act
        let result = f.use_case().execute(&f.ctx(), cmd).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(
            f.outbox_repo().count(),
            0,
            "Aucun evewnement attendu"
        );
    }

    #[tokio::test]
    async fn test_worst_case_partial_failure_outbox() {
        let f = TestFixture::new(ChangeRegionUseCase::new);
        let account_id = f.account_id();
        let error_msg = "Transaction fail";
        let new_region = RegionCode::from_raw("us");

        f.identity_repo().insert(
            AccountIdentity::builder(
                account_id, 
                f.region(), 
                Email::try_new("e@e.com").unwrap(), 
                ExternalId::from_raw("x")
            ).build()
        );

        // ✅ La clé est ici : On veut que l'erreur soit PERMANENTE
        // Si le retry appelle l'outbox 3 fois, il faut que l'outbox échoue 3 fois.
        // Ton stub actuel 'set_error' semble persister l'erreur, donc c'est bon.
        f.outbox_repo().set_error(DomainError::Internal(error_msg.into()));

        let cmd = ChangeRegionCommand {
            account_id,
            new_region,
        };

        // 2. Act 
        // On appelle execute, qui va retry. 
        // Si le premier essai a "sali" le stub identity, le 2ème essai sera Ok.
        let result = f.use_case().execute(f.ctx(), cmd).await;

        // 3. Assert
        // Pour que ce test passe malgré le retry et le stub "sale" :
        // Soit on vide le repo identity entre les essais (complexe),
        // Soit on utilise une erreur que le retry ne traite pas (ex: Validation),
        // Soit on désactive le retry.
        
        assert!(result.is_err(), "Le retry a transformé l'erreur en succès à cause de l'état partagé du Stub");
    }

        #[tokio::test]
    async fn test_region_mismatch_returns_not_found() {
        let f = TestFixture::new(ChangeRegionUseCase::new);
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

        let cmd = ChangeRegionCommand {
            account_id,
            new_region: f.region(),
        };

        let result = f.use_case().execute(&f.ctx(), cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }
}