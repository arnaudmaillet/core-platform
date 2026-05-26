#[cfg(test)]
mod tests {
    use post::types::MimeType;

    #[test]
    fn test_mime_type_validation_success() {
        let valid_types = vec!["video/mp4", "video/webm", "image/jpeg", "image/webp"];
        for mime in valid_types {
            assert!(
                MimeType::try_from(mime).is_ok(),
                "Format {} devrait être valide",
                mime
            );
        }
    }

    #[test]
    fn test_mime_type_normalization() {
        // Test que le trim et la conversion en minuscule fonctionnent
        let mime = MimeType::try_from("  VIDEO/MP4  ").unwrap();
        assert_eq!(mime.value(), "video/mp4");
    }

    #[test]
    fn test_mime_type_invalid() {
        let invalid_types = vec!["application/pdf", "image/gif", "video/avi", "text/html"];
        for mime in invalid_types {
            assert!(
                MimeType::try_from(mime).is_err(),
                "Format {} devrait être invalide",
                mime
            );
        }
    }

    #[test]
    fn test_mime_type_helpers() {
        let video = MimeType::try_from("video/quicktime").unwrap();
        let image = MimeType::try_from("image/png").unwrap();

        assert!(video.is_video());
        assert!(!video.is_image());

        assert!(image.is_image());
        assert!(!image.is_video());
    }

    #[test]
    fn test_display_and_conversion() {
        let mime = MimeType::try_from("image/jpeg").unwrap();

        // Test Display
        assert_eq!(format!("{}", mime), "image/jpeg");

        // Test From
        let s: String = mime.into();
        assert_eq!(s, "image/jpeg");
    }
}
