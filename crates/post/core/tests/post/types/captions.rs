mod tests {
    use post::Caption;
    use std::str::FromStr;

    #[test]
    fn test_caption_success() {
        let text = "Ceci est une légende valide.".to_string();
        let caption = Caption::try_new(text.clone()).expect("Devrait être valide");
        assert_eq!(caption.value(), text.trim());
        assert!(!caption.is_empty());
    }

    #[test]
    fn test_caption_trimming() {
        let text = "  Légende avec espaces  ";
        let caption = Caption::try_new(text.to_string()).unwrap();
        assert_eq!(caption.value(), "Légende avec espaces");
    }

    #[test]
    fn test_caption_too_long_fails() {
        let long_text = "a".repeat(Caption::MAX_LENGTH + 1);
        let result = Caption::try_new(long_text);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_hashtags() {
        let caption = Caption::try_new("J'adore #rust et le #devops ! #123".to_string()).unwrap();
        let tags = caption.extract_hashtags();

        assert!(tags.contains("rust"));
        assert!(tags.contains("devops"));
        assert!(tags.contains("123"));
        assert_eq!(tags.len(), 3);
    }

    #[test]
    fn test_extract_mentions() {
        let caption =
            Caption::try_new("Salut @arnaud_dev, rejoins @juliette !".to_string()).unwrap();
        let mentions = caption.extract_mentions();

        assert!(mentions.contains("arnaud_dev"));
        assert!(mentions.contains("juliette"));
        assert_eq!(mentions.len(), 2);
    }

    #[test]
    fn test_conversions() {
        // Test TryFrom String
        let s = "Valid".to_string();
        let c = Caption::try_from(s.clone()).unwrap();
        let back_to_string: String = c.into();
        assert_eq!(back_to_string, s);

        // Test FromStr
        let c2 = Caption::from_str("Test").unwrap();
        assert_eq!(c2.value(), "Test");
    }

    #[test]
    fn test_edge_case_tags_mentions() {
        // Test ponctuation et cas limites
        let caption = Caption::try_new("#Rust! @user?".to_string()).unwrap();

        let tags = caption.extract_hashtags();
        assert!(tags.contains("rust"));

        let mentions = caption.extract_mentions();
        assert!(mentions.contains("user"));
    }
}
