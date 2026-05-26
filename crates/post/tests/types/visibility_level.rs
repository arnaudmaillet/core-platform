#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use post::types::VisibilityLevel;

    #[test]
    fn test_visibility_level_from_str() {
        assert_eq!(
            VisibilityLevel::from_str("public").unwrap(),
            VisibilityLevel::Public
        );
        assert_eq!(
            VisibilityLevel::from_str("FRIENDS").unwrap(),
            VisibilityLevel::Friends
        );
        assert_eq!(
            VisibilityLevel::from_str("  subscribers  ").unwrap(),
            VisibilityLevel::Subscribers
        );
        assert_eq!(
            VisibilityLevel::from_str("private").unwrap(),
            VisibilityLevel::Private
        );
    }

    #[test]
    fn test_visibility_level_invalid() {
        assert!(VisibilityLevel::from_str("everyone").is_err());
        assert!(VisibilityLevel::from_str("hidden").is_err());
    }

    #[test]
    fn test_visibility_helpers() {
        let public = VisibilityLevel::Public;
        let sub = VisibilityLevel::Subscribers;
        let private = VisibilityLevel::Private;

        // Test monétisation
        assert!(sub.is_monetized());
        assert!(!public.is_monetized());

        // Test découvrabilité (FYP)
        assert!(public.is_discoverable());
        assert!(!private.is_discoverable());
        assert!(!sub.is_discoverable());
    }

    #[test]
    fn test_conversions() {
        // String -> VisibilityLevel
        let v = VisibilityLevel::try_from("friends".to_string()).unwrap();
        assert_eq!(v, VisibilityLevel::Friends);

        // VisibilityLevel -> String
        let s: String = VisibilityLevel::Private.into();
        assert_eq!(s, "private");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", VisibilityLevel::Public), "public");
    }
}
