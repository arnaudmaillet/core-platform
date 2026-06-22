#[cfg(test)]
mod tests {
    use post_older::DynamicMetadata;
    use serde::Serialize;
    use serde_json::json;

    #[test]
    fn test_dynamic_metadata_valid() {
        let json = json!({"foo": "bar", "count": 1});
        let meta = DynamicMetadata::try_new(json).expect("Devrait être valide");
        assert!(meta.value().is_object());
    }

    #[test]
    fn test_dynamic_metadata_invalid_root() {
        // Le root doit être un objet, pas un array
        let json = json!([1, 2, 3]);
        assert!(DynamicMetadata::try_new(json).is_err());
    }

    #[test]
    fn test_size_limit_enforcement() {
        // Création d'une charge utile dépassant 64 Ko
        let large_string = "a".repeat(DynamicMetadata::MAX_SIZE_BYTES + 1);
        let json = json!({"data": large_string});

        let result = DynamicMetadata::try_new(json);
        assert!(result.is_err(), "La taille devrait dépasser la limite");
    }

    #[test]
    fn test_feature_management() {
        let mut meta = DynamicMetadata::empty();

        #[derive(Serialize, serde::Deserialize, PartialEq, Debug)]
        struct TestFeature {
            score: i32,
        }

        let feature = TestFeature { score: 42 };

        // Ajout
        meta.with_feature("game", feature).unwrap();

        // Lecture
        let retrieved: TestFeature = meta.get_feature("game").unwrap();
        assert_eq!(retrieved.score, 42);

        // Erreur si clé inexistante
        let missing: std::result::Result<TestFeature, _> = meta.get_feature("unknown");
        assert!(missing.is_err());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let original_json = json!({"theme": "dark", "version": 1});
        let meta = DynamicMetadata::try_new(original_json.clone()).unwrap();

        // Test From/Into String
        let serialized: String = meta.into();
        let deserialized = DynamicMetadata::try_from(serialized).unwrap();

        assert_eq!(deserialized.value(), &original_json);
    }
}
