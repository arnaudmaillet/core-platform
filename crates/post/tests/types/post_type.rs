#[cfg(test)]
mod tests {
    use shared_kernel::types::PostType;
    use std::str::FromStr;

    #[test]
    fn test_post_type_from_str() {
        assert_eq!(PostType::from_str("video").unwrap(), PostType::Video);
        assert_eq!(PostType::from_str("CAROUSEL").unwrap(), PostType::Carousel);
        assert_eq!(PostType::from_str("  image  ").unwrap(), PostType::Image);
        assert_eq!(PostType::from_str("text").unwrap(), PostType::Text);
    }

    #[test]
    fn test_post_type_invalid() {
        assert!(PostType::from_str("live").is_err());
        assert!(PostType::from_str("").is_err());
    }

    #[test]
    fn test_post_type_helpers() {
        let text = PostType::Text;
        let video = PostType::Video;
        let carousel = PostType::Carousel;

        // Vérification des besoins en média
        assert!(!text.requires_media());
        assert!(video.requires_media());
        assert!(carousel.requires_media());

        // Vérification du type carousel
        assert!(carousel.is_carousel());
        assert!(!video.is_carousel());
    }

    #[test]
    fn test_conversions() {
        // String -> PostType
        let p = PostType::try_from("video".to_string()).unwrap();
        assert_eq!(p, PostType::Video);

        // PostType -> String
        let s: String = PostType::Text.into();
        assert_eq!(s, "text");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", PostType::Video), "video");
        assert_eq!(format!("{}", PostType::Carousel), "carousel");
    }
}
