#[cfg(test)]
mod tests {
    use crate::domain::entities::Account;
    use crate::types::{RegistrationIdentifier, TrustAmount, TrustScore};
    use shared_kernel::core::{AggregateRoot, ErrorCode, Result};
    use shared_kernel::geo::Timezone;
    use shared_kernel::messaging::EventEmitter;
    use shared_kernel::security::PushToken;
    use shared_kernel::types::{AccountId, AuditReason, Email, Region, SubId};

    /// Helper pour créer un compte de test valide et actif
    fn create_test_account() -> Account {
        let id = AccountId::generate(Region::default());
        let identifier =
            RegistrationIdentifier::from_email(Email::try_new("john@doe.com").unwrap());

        Account::builder(id, identifier)
            .build()
            .expect("Failed to build test account")
    }

    // #[test]
    // fn test_registration_flow() -> Result<()> {
    //     let mut account = create_test_account();
    //     let ip = IpAddr::try_new("127.0.0.1")?;

    //     // Act
    //     account.register(RegionCode::try_new("EU")?, ip.clone())?;

    //     // Assert
    //     assert_eq!(account.governance().last_ip_addr(), Some(&ip));
    //     assert_eq!(account.metadata().version(), 1);

    //     let events = account.pull_events();
    //     assert_eq!(events.len(), 1);
    //     Ok(())
    // }

    #[test]
    fn test_penalize_trust_with_automated_shadowban() -> Result<()> {
        let mut account = create_test_account();
        let reason = AuditReason::try_new("Repeated violations")?;
        let version_init = account.metadata().version();

        // Act: On inflige une pénalité qui fait tomber le score à 0
        // Le score initial est 100. Une pénalité de 100 active le shadowban auto.
        account.penalize_trust(TrustAmount::PENALTY_BAN, reason)?;

        // Assert
        assert_eq!(account.governance().trust_score().value(), 0);
        assert!(account.governance().is_shadowbanned());

        // Version: Init(0) + 1 (Penalty/TrackChange) = 1
        // Note: Si penalize_trust appelle shadowban() en interne via track_change,
        // la version monterait à 2. Avec notre implémentation "extra_event", elle reste à 1.
        assert_eq!(account.metadata().version(), version_init + 1);

        // Events: TrustScoreAdjusted + ShadowbanUpdated
        let events = account.pull_events();
        assert_eq!(events.len(), 2);

        assert!(
            account
                .governance()
                .moderation_notes()
                .unwrap()
                .contains("Trust score critical threshold reached")
        );

        Ok(())
    }

    #[test]
    fn test_idempotency_at_score_floor() -> Result<()> {
        let mut account = create_test_account();
        let reason = AuditReason::try_new("Test floor")?;

        // On met d'abord le compte au plancher
        account.penalize_trust(TrustAmount::PENALTY_BAN, reason.clone())?;
        account.pull_events();
        let version_before = account.metadata().version();

        let penalty_amount = TrustAmount::try_from(10)?;
        let changed = account.penalize_trust(penalty_amount, reason)?;

        // Assert
        assert!(!changed);
        assert_eq!(
            account.metadata().version(),
            version_before,
            "La version ne doit pas changer"
        );
        assert_eq!(account.pull_events().len(), 0);
        Ok(())
    }

    #[test]
    fn test_ban_and_unban_impact_on_trust() -> Result<()> {
        let mut account = create_test_account();
        let reason = AuditReason::try_new("Toxic behavior")?;

        // 1. État Initial : Score MAX (100)
        assert_eq!(account.governance().trust_score().value(), TrustScore::MAX);

        // 2. Ban
        // Selon tes constantes : PENALTY_BAN = 100
        account.ban(reason.clone())?;
        assert!(account.identity().is_banned());

        // 100 - 100 = 0
        assert_eq!(account.governance().trust_score().value(), TrustScore::MIN);

        // 3. Unban
        // Selon tes constantes : REWARD_UNBAN = 20
        account.unban(reason)?;
        assert!(!account.identity().is_banned());

        // 0 + 20 = 20
        assert_eq!(
            account.governance().trust_score(),
            TrustScore::from_raw(TrustScore::CRITICAL_THRESHOLD)
        );

        Ok(())
    }

    #[test]
    fn test_ensure_not_restricted_guard() -> Result<()> {
        let mut account = create_test_account();
        account.ban(AuditReason::system("Banned for test"))?;

        // Act: Essayer de changer l'email sur un compte banni
        let result = account.change_email(Email::try_new("new@email.com")?);

        // Assert
        assert!(result.is_err());
        match result {
            Err(e) => assert_eq!(e.code, ErrorCode::Forbidden),
            Ok(_) => panic!("Should have returned a Forbidden error"),
        }
        Ok(())
    }

    #[test]
    fn test_update_timezone_validation() -> Result<()> {
        let mut account = create_test_account();
        let valid_tz = Timezone::try_new("Europe/Paris")?;

        // Act
        let changed = account.update_timezone(valid_tz)?;

        // Assert
        assert!(changed);
        assert_eq!(account.settings().timezone().as_str(), "Europe/Paris");
        Ok(())
    }

    #[test]
    fn test_activity_throttling_logic() -> Result<()> {
        let mut account = create_test_account();

        // Premier record (None -> Some)
        let first = account.record_activity()?;
        assert!(first);

        // Deuxième record immédiat (Throttle < 5min)
        let second = account.record_activity()?;
        assert!(!second);

        Ok(())
    }

    #[test]
    fn test_link_sub_identity_forbidden_if_already_linked() -> Result<()> {
        // 1. Arrange : Un compte qui a DEJÀ un lien externe
        let mut account = create_test_account();
        let initial_ext = SubId::try_new("google|123")?;

        // On lie le premier ID (ceci doit réussir)
        account.link_sub_identity(initial_ext)?;

        // 2. Act : On tente d'en lier un DEUXIÈME (différent)
        let new_ext = SubId::try_new("apple|456")?;
        let result = account.link_sub_identity(new_ext);

        // 3. Assert : L'agrégat doit refuser (Forbidden)
        assert!(result.is_err());

        match result {
            Err(e) if e.code == ErrorCode::Forbidden => {
                assert!(e.message.contains("already linked"));
            }
            _ => panic!("Expected Forbidden error, got {:?}", result),
        }

        Ok(())
    }

    #[test]
    fn test_automated_shadowban_on_critical_score() -> Result<()> {
        // 1. Arrange : On utilise l'agrégat complet
        let mut account = create_test_account();
        let reason = AuditReason::try_new("Major violation")?;

        // 2. Act : On passe par la méthode de la racine
        // On inflige une pénalité qui fait tomber le score à 0 (Score initial 100 - 150)
        let penalty_amount = TrustAmount::try_from(150)?;
        let changed = account.penalize_trust(penalty_amount, reason)?;

        // 3. Assert
        assert!(changed);

        let gov = account.governance();
        assert_eq!(gov.trust_score().value(), 0);

        // C'est maintenant Account qui a déclenché le shadowban !
        assert!(
            gov.is_shadowbanned(),
            "Le shadowban automatique aurait dû être activé par l'agrégat"
        );

        // On vérifie que la note de modération contient le message défini dans Account
        assert!(
            gov.moderation_notes()
                .unwrap()
                .contains("Trust score critical threshold reached")
        );

        Ok(())
    }

    #[test]
    fn test_push_token_lifecycle() -> Result<()> {
        let mut account = create_test_account();
        let token = PushToken::try_new("token_123")?;

        // 1. Ajout
        let added = account.add_push_token(token.clone())?;
        assert!(added);
        assert_eq!(account.settings().push_tokens().len(), 1);

        // Vérification de l'idempotence (ajout du même token)
        let added_again = account.add_push_token(token.clone())?;
        assert!(!added_again);

        // 2. Suppression
        let removed = account.remove_push_token(token.clone())?;
        assert!(removed);
        assert_eq!(account.settings().push_tokens().len(), 0);

        // Idempotence suppression
        let removed_again = account.remove_push_token(token)?;
        assert!(!removed_again);

        Ok(())
    }
}
