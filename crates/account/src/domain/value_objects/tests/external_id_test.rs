#[cfg(test)]
mod tests {
    use crate::domain::value_objects::external_id::ExternalId;
    use shared_kernel::domain::value_objects::ValueObject;
    use shared_kernel::errors::DomainError;
    use std::str::FromStr;

    #[test]
    fn test_external_id_happy_path() {
        // Un ID classique venant d'un sub OIDC (Google/GitHub)
        let valid_ids = vec![
            "1234567890",
            "auth0|654321",
            "google-oauth2|10630123456789",
            "github_user_88",
            "A-Z_0-9.important-id",
        ];

        for id in valid_ids {
            let res = ExternalId::try_new(id);
            assert!(res.is_ok(), "Should be valid: {}", id);
            assert_eq!(res.unwrap().as_str(), id);
        }
    }

    #[test]
    fn test_external_id_trimming() {
        // On vérifie que les espaces accidentels sont nettoyés
        let id = ExternalId::try_new("  google|123  ").unwrap();
        assert_eq!(id.as_str(), "google|123");
    }

    #[test]
    fn test_external_id_empty_fails() {
        let res = ExternalId::try_new("");
        assert!(matches!(res, Err(DomainError::Validation { field, .. }) if field == "external_id"));

        let res_space = ExternalId::try_new("   ");
        assert!(res_space.is_err(), "Only spaces should be considered empty after trim");
    }

    #[test]
    fn test_external_id_too_long() {
        // On teste la limite de 128 caractères
        let very_long_id = "a".repeat(129);
        let res = ExternalId::try_new(very_long_id);

        assert!(res.is_err());
        if let Err(DomainError::Validation { reason, .. }) = res {
            assert!(reason.contains("suspiciously long"));
        }
    }

    #[test]
    fn test_external_id_from_raw_skips_validation() {
        // En infrastructure, on doit pouvoir reconstruire même si c'est "sale"
        // (ex: si on change la règle de validation plus tard)
        let raw = ExternalId::from_raw("");
        assert_eq!(raw.as_str(), "");
        // Par contre le validate() manuel doit échouer
        assert!(raw.validate().is_err());
    }

    #[test]
    fn test_external_id_conversions() {
        // Test FromStr
        let id = ExternalId::from_str("provider|abc").unwrap();
        assert_eq!(id.as_str(), "provider|abc");

        // Test Display
        assert_eq!(format!("{}", id), "provider|abc");

        // Test String Conversion
        let s: String = id.into();
        assert_eq!(s, "provider|abc");
    }
}