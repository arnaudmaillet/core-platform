#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use post_older::MediaId;
    use shared_kernel::core::{Identifier, ValueObject};
    use uuid::Uuid;

    #[test]
    fn test_media_id_generation() {
        // Test que la génération produit bien un UUIDv4
        let id = MediaId::generate();
        assert_eq!(id.uuid().get_version_num(), 4);
    }

    #[test]
    fn test_media_id_from_valid_uuid4() {
        let uuid = Uuid::new_v4();
        let id = MediaId::new(uuid);
        assert!(id.validate().is_ok());
    }

    #[test]
    fn test_media_id_invalid_version() {
        let uuid_v7 = Uuid::now_v7();
        let id = MediaId::new(uuid_v7);

        let result = id.validate();
        assert!(result.is_err());

        let err = result.unwrap_err();
        let details = err
            .details
            .as_ref()
            .expect("L'erreur devrait avoir des détails");

        assert!(details["reason"].as_str().unwrap().contains("UUIDv4"));
    }

    #[test]
    fn test_media_id_nil() {
        let id = MediaId::new(Uuid::nil());
        assert!(id.validate().is_err());
    }

    #[test]
    fn test_from_str_conversion() {
        // Test parsing valide
        let uuid_str = Uuid::new_v4().to_string();
        let id = MediaId::from_str(&uuid_str).unwrap();
        assert_eq!(id.to_string(), uuid_str);

        // Test format invalide
        assert!(MediaId::from_str("invalid-uuid").is_err());
    }

    #[test]
    fn test_identifier_trait() {
        let id = MediaId::generate();
        assert_eq!(id.as_uuid(), id.uuid());
        assert_eq!(id.as_string(), id.to_string());
    }
}
