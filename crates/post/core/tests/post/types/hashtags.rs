#[cfg(test)]
mod tests {
    use post::Hashtags;

    #[test]
    fn test_hashtags_validation_success() {
        let tags = vec![
            "#rust".to_string(),
            "devops".to_string(),
            "#Wynn_2026".to_string(),
        ];
        let h = Hashtags::try_from(tags).expect("Devrait être valide");

        assert_eq!(h.len(), 3);
        assert!(h.contains("rust"));
        assert!(h.contains("wynn_2026"));
    }

    #[test]
    fn test_hashtags_normalization() {
        let tags = vec![
            "#RUST".to_string(),
            "rust".to_string(),
            "  Rust  ".to_string(),
        ];
        let h = Hashtags::try_from(tags).unwrap();

        assert_eq!(h.len(), 1);
        assert!(h.contains("rust"));
    }

    #[test]
    fn test_max_tags_count_exceeded() {
        let many_tags: Vec<String> = (0..21).map(|i| format!("tag{}", i)).collect();
        let result = Hashtags::try_from(many_tags);

        assert!(result.is_err());

        let err = result.unwrap_err();
        let details = err
            .details
            .as_ref()
            .expect("L'erreur devrait avoir des détails");

        assert!(
            details["reason"]
                .as_str()
                .unwrap()
                .contains("cannot have more than 20")
        );
    }

    #[test]
    fn test_tag_too_long() {
        let long_tag = "a".repeat(Hashtags::MAX_TAG_LENGTH + 1);
        let result = Hashtags::try_from(vec![long_tag]);

        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_characters() {
        // Caractères spéciaux interdits
        let invalid_tags = vec![
            "rust!".to_string(),
            "c#".to_string(),
            "space in tag".to_string(),
        ];
        for tag in invalid_tags {
            assert!(
                Hashtags::try_from(vec![tag]).is_err(),
                "Le tag devrait être invalide"
            );
        }
    }

    #[test]
    fn test_empty_tags_handling() {
        let tags = vec!["#".to_string(), "   ".to_string()];
        let h = Hashtags::try_from(tags).unwrap();

        assert!(h.is_empty());
    }
}
