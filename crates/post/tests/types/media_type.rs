#[cfg(test)]
mod tests {
    use post::types::MediaType;
    use std::str::FromStr;

    #[test]
    fn test_media_type_from_str() {
        // Succès
        assert_eq!(MediaType::from_str("video").unwrap(), MediaType::Video);
        assert_eq!(MediaType::from_str("image").unwrap(), MediaType::Image);
        assert_eq!(MediaType::from_str("  video  ").unwrap(), MediaType::Video);
    }

    #[test]
    fn test_media_type_invalid() {
        // Erreur
        assert!(MediaType::from_str("gif").is_err());
        assert!(MediaType::from_str("").is_err());
    }

    #[test]
    fn test_media_type_helpers() {
        let video = MediaType::Video;
        let image = MediaType::Image;

        assert!(video.is_video());
        assert!(!video.is_image());

        assert!(image.is_image());
        assert!(!image.is_video());
    }

    #[test]
    fn test_conversions() {
        // String -> MediaType
        let m = MediaType::try_from("video".to_string()).unwrap();
        assert_eq!(m, MediaType::Video);

        // MediaType -> String
        let s: String = MediaType::Image.into();
        assert_eq!(s, "image");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", MediaType::Video), "video");
        assert_eq!(format!("{}", MediaType::Image), "image");
    }
}
