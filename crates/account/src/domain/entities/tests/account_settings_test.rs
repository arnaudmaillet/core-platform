#[cfg(test)]
mod tests {
    use chrono::Utc;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, PushToken, Timezone};
    use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
    use shared_kernel::errors::DomainError;
    use crate::domain::entities::{AccountSettings, AppearanceSettings, NotificationSettings, PrivacySettings};
    use crate::domain::entities::account_settings::ThemeMode;

    // Helper pour initialiser un AccountSettings de test
    fn create_test_settings() -> AccountSettings {
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();

        AccountSettings::restore(
            account_id,
            region,
            PrivacySettings::default(),
            NotificationSettings::default(),
            AppearanceSettings::default(),
            Timezone::try_new("UTC").unwrap(),
            vec![],
            Utc::now(),
            AggregateMetadata::default(),
        )
    }

    #[test]
    fn test_timezone_update_consistency() {
        let mut settings = create_test_settings();
        let region = settings.region_code().clone();
        let new_tz = Timezone::try_new("Europe/Paris").unwrap();

        // 1. Premier changement : Doit être TRUE
        let changed = settings.update_timezone(&region, new_tz.clone()).unwrap();
        assert!(changed, "First change should return true");
        assert_eq!(settings.metadata_mut().pull_events().len(), 1);

        // 2. Deuxième changement (même valeur) : Doit être FALSE
        let changed_again = settings.update_timezone(&region, new_tz).unwrap();
        assert!(!changed_again, "Redundant update should return false"); // FIX ICI
        assert_eq!(settings.metadata_mut().pull_events().len(), 0); // FIX ICI
    }

    #[test]
    fn test_push_token_fifo_rotation() {
        let mut settings = create_test_settings();
        let region = settings.region_code().clone();

        // On remplit jusqu'à la limite (10)
        for i in 0..10 {
            let token_str = format!("push_token_valide_{}", i);
            let token = PushToken::try_new(token_str).unwrap();
            let changed = settings.add_push_token(&region, token).unwrap();
            assert!(changed);
        }

        let _ = settings.metadata_mut().pull_events();

        // On ajoute le 11ème (doit éjecter le 0)
        let token_11 = PushToken::try_new("push_token_valide_11").unwrap();
        let changed = settings.add_push_token(&region, token_11).unwrap();

        assert!(changed);
        assert_eq!(settings.push_tokens().len(), 10);
        assert_eq!(settings.push_tokens()[0].as_str(), "push_token_valide_1");
        assert_eq!(settings.metadata_mut().pull_events().len(), 1);
    }

    #[test]
    fn test_push_token_removal_and_idempotency() {
        let mut settings = create_test_settings();
        let region = settings.region_code().clone();
        let token = PushToken::try_new("token_to_delete_long_enough").unwrap();

        // 1. On ajoute : OK
        settings.add_push_token(&region, token.clone()).unwrap();
        let _ = settings.metadata_mut().pull_events(); // On nettoie l'outbox

        // 2. Action de suppression réelle : doit être true (1 event)
        let changed = settings.remove_push_token(&region, &token).unwrap();
        assert!(changed);
        assert_eq!(settings.push_tokens().len(), 0);
        assert_eq!(settings.metadata_mut().pull_events().len(), 1);

        // 3. Deuxième suppression : doit être false (0 event)
        let changed_again = settings.remove_push_token(&region, &token).unwrap();
        assert!(!changed_again);
        assert_eq!(settings.metadata_mut().pull_events().len(), 0);
    }

    #[test]
    fn test_update_preferences_partial_and_idempotency() {
        let mut settings = create_test_settings();
        let region = settings.region_code().clone();

        let new_appearance = AppearanceSettings {
            theme: ThemeMode::Dark,
            high_contrast: true,
        };

        // 1. Update partielle : true
        let changed = settings.update_preferences(&region, None, None, Some(new_appearance.clone())).unwrap();
        assert!(changed);
        assert_eq!(settings.appearance(), &new_appearance);

        let _ = settings.metadata_mut().pull_events();

        // 2. Update avec les mêmes valeurs (Idempotence) : false
        let changed = settings.update_preferences(&region, None, None, Some(new_appearance)).unwrap();
        assert!(!changed);
        assert_eq!(settings.metadata_mut().pull_events().len(), 0);
    }

    #[test]
    fn test_cross_region_settings_security() {
        let mut settings = create_test_settings();
        let wrong_region = RegionCode::try_new("us").unwrap();
        let token = PushToken::try_new("some_valid_token").unwrap();

        // Doit renvoyer Err(Forbidden)
        let result = settings.add_push_token(&wrong_region, token);
        assert!(result.is_err());
    }

    #[test]
    fn test_change_region_idempotency() {
        let mut settings = create_test_settings();
        let new_region = RegionCode::try_new("us").unwrap();

        // Premier changement : true
        let changed = settings.change_region(new_region.clone()).unwrap();
        assert!(changed);
        assert_eq!(settings.region_code(), &new_region);

        // Deuxième fois : false
        let changed = settings.change_region(new_region).unwrap();
        assert!(!changed);
    }
}