#[cfg(test)]
mod tests {
    use crate::domain::value_objects::locale::Locale;
    use shared_kernel::domain::value_objects::ValueObject;
    use std::str::FromStr;

    #[test]
    fn test_locale_happy_path() {
        let valid_locales = vec![
            "en",         // Langue seule
            "fr-FR",      // Langue + Région
            "zh-Hans",    // Langue + Script
            "en-US-posix" // Cas long mais valide (11 chars ? Ah, attention à ta limite de 10 !)
        ];

        for l in valid_locales {
            // Note: Si "en-US-posix" échoue, c'est à cause de ta limite à 10.
            // Pour BCP-47, 10 est parfois juste, mais suffisant pour 99% des cas.
            let res = Locale::try_new(l);
            assert!(res.is_ok(), "Should be valid: {}", l);
        }
    }

    #[test]
    fn test_locale_normalization() {
        // Test du remplacement de l'underscore par le tiret (très fréquent sur mobile)
        let l = Locale::try_new("fr_FR").unwrap();
        assert_eq!(l.as_str(), "fr-FR");

        // Test du trim
        let l2 = Locale::try_new("  en-GB  ").unwrap();
        assert_eq!(l2.as_str(), "en-GB");
    }

    #[test]
    fn test_locale_language_code_extraction() {
        let l = Locale::try_new("fr-CA").unwrap();
        assert_eq!(l.language_code(), "fr");

        let l2 = Locale::try_new("en").unwrap();
        assert_eq!(l2.language_code(), "en");
    }

    #[test]
    fn test_locale_invalid_formats() {
        let cases = vec![
            ("f", "too short"),
            ("this-is-way-too-long", "too long"),
            ("fr.FR", "invalid character (dot)"),
            ("fr!FR", "invalid character (!)"),
            ("é-FR", "non-ascii character"),
        ];

        for (input, reason) in cases {
            assert!(
                Locale::try_new(input).is_err(),
                "Should fail for {}: {}", input, reason
            );
        }
    }

    #[test]
    fn test_locale_default() {
        let d = Locale::default();
        assert_eq!(d.as_str(), "en-US");
    }

    #[test]
    fn test_locale_conversions() {
        // FromStr
        let l = Locale::from_str("de-DE").unwrap();
        assert_eq!(l.as_str(), "de-DE");

        // Display
        assert_eq!(format!("{}", l), "de-DE");

        // Into String
        let s: String = l.into();
        assert_eq!(s, "de-DE");
    }

    #[test]
    fn test_locale_from_raw_skips_validation() {
        // Cas d'usage : On charge une vieille locale erronée en DB
        let raw = Locale::from_raw("x");
        assert_eq!(raw.as_str(), "x");
        assert!(raw.validate().is_err());
    }
}