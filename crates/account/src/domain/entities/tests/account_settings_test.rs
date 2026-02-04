#[cfg(test)]
mod tests {
    use chrono::Utc;
    use super::*;
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
            Timezone::try_new("Europe/Paris").unwrap(),
            vec![],
            Utc::now(),
            AggregateMetadata::default(),
        )
    }

    #[test]
    fn test_timezone_update_consistency() {
        let mut settings = create_test_settings();

        // Cas 1 : Update valide
        let valid_tz = Timezone::try_new("Europe/London").unwrap();
        assert!(settings.update_timezone(valid_tz).is_ok());

        // Cas 2 : Update invalide (Garde métier : Région EU vs Timezone America)
        let invalid_tz = Timezone::try_new("America/New_York").unwrap();
        let result = settings.update_timezone(invalid_tz);

        assert!(matches!(result, Err(DomainError::Validation { field, .. }) if field == "timezone"));

        // Cas 3 : Idempotence
        let current_tz = settings.timezone().clone();
        let event_count_before = settings.metadata_mut().pull_events().len();
        settings.update_timezone(current_tz).unwrap();
        assert_eq!(settings.metadata_mut().pull_events().len(), 0);
    }

    #[test]
    fn test_push_token_fifo_rotation() {
        let mut settings = create_test_settings();

        // On remplit jusqu'à la limite (10) avec des tokens valides (> 8 chars)
        for i in 0..10 {
            let token_str = format!("push_token_valide_{}", i);
            settings.add_push_token(PushToken::try_new(token_str).unwrap()).unwrap();
        }

        // On vide la file d'événements des 10 ajouts précédents
        let _ = settings.metadata_mut().pull_events();

        // On ajoute le 11ème (doit éjecter le 0)
        let token_11 = PushToken::try_new("push_token_valide_11").unwrap();
        settings.add_push_token(token_11).unwrap();

        assert_eq!(settings.push_tokens().len(), 10);

        // Le premier doit maintenant être le "1" (le "0" a été remove(0))
        assert_eq!(settings.push_tokens()[0].as_str(), "push_token_valide_1");

        // Vérification de l'événement unique
        assert_eq!(settings.metadata_mut().pull_events().len(), 1);
    }

    #[test]
    fn test_push_token_removal() {
        let mut settings = create_test_settings();
        let token = PushToken::try_new("token_to_delete").unwrap();

        settings.add_push_token(token.clone()).unwrap();

        // ON VIDE pour ne pas compter l'ajout
        let _ = settings.metadata_mut().pull_events();

        // Action de suppression
        settings.remove_push_token(&token).unwrap();

        assert_eq!(settings.push_tokens().len(), 0);
        assert_eq!(settings.metadata_mut().pull_events().len(), 1, "Devrait avoir 1 event de suppression");

        // Deuxième suppression (Idempotence)
        settings.remove_push_token(&token).unwrap();
        assert_eq!(settings.metadata_mut().pull_events().len(), 0, "Rien ne doit être généré la 2ème fois");
    }

    #[test]
    fn test_update_preferences_partial() {
        let mut settings = create_test_settings();

        let new_appearance = AppearanceSettings {
            theme: ThemeMode::Dark,
            high_contrast: true,
        };

        // On ne met à jour QUE l'apparence
        settings.update_preferences(None, None, Some(new_appearance.clone())).unwrap();

        assert_eq!(settings.appearance(), &new_appearance);
        assert_eq!(settings.privacy(), &PrivacySettings::default()); // Inchangé

        let events = settings.metadata_mut().pull_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_update_preferences_idempotency() {
        let mut settings = create_test_settings();
        let current_privacy = settings.privacy().clone();

        // Update avec les mêmes valeurs
        settings.update_preferences(Some(current_privacy), None, None).unwrap();

        let events = settings.metadata_mut().pull_events();
        assert_eq!(events.len(), 0, "No event should be fired if nothing changed");
    }

    #[test]
    fn test_change_region_logic() {
        let mut settings = create_test_settings();
        let new_region = RegionCode::try_new("us").unwrap();

        settings.change_region(new_region.clone()).unwrap();
        assert_eq!(settings.region_code(), &new_region);
        assert_eq!(settings.metadata().version(), 2);
    }
}