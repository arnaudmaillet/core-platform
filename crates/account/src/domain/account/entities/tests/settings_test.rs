#[cfg(test)]
mod tests {
    use crate::domain::account::entities::{AccountSettings, AccountPreferences};
    use crate::domain::preferences::models::{
        AppearancePreferences, NotificationPreferences,
        PrivacyPreferences,
    };
    use chrono::Utc;
    use shared_kernel::domain::events::{AggregateMetadata, AggregateRoot};
    use shared_kernel::domain::value_objects::{AccountId, PushToken, RegionCode, Timezone};

    // Helper pour initialiser un AccountSettings de test
    fn create_test_settings() -> AccountSettings {
        let account_id = AccountId::new();
        let preferences = AccountPreferences::new(
            PrivacyPreferences::default(),
            NotificationPreferences::default(),
            AppearancePreferences::default(),
        );

        AccountSettings::restore(
            account_id,
            preferences, // On passe l'objet groupé ici
            Timezone::try_new("UTC").unwrap(),
            vec![],
            Utc::now(),
            AggregateMetadata::default(),
        )
    }

    #[test]
    fn test_timezone_update_idempotency() {
        let mut settings = create_test_settings();
        let region = RegionCode::try_new("eu").unwrap(); // On définit une région cohérente
        let new_tz = Timezone::try_new("Europe/Paris").unwrap();

        // 1. Premier passage : on fournit la région
        let changed = settings.update_timezone(new_tz.clone(), &region).unwrap();
        assert!(changed);
        // On vérifie qu'un événement a été généré
        assert_eq!(settings.metadata_mut().pull_events().len(), 1);

        // 2. Idempotence : même valeur + même région
        let changed_again = settings.update_timezone(new_tz, &region).unwrap();
        assert!(!changed_again);
        // Aucun nouvel événement ne doit être présent
        assert_eq!(settings.metadata_mut().pull_events().len(), 0);
    }

    #[test]
    fn test_push_token_fifo_rotation() {
        let mut settings = create_test_settings();

        // 1. On utilise des tokens d'au moins 8 caractères
        for i in 0..10 {
            // "push_token_0" fait 12 caractères -> OK
            let token = PushToken::try_new(format!("push_token_{}", i)).unwrap();
            settings.add_push_token(token).unwrap();
        }

        // 2. On vérifie la rotation avec un 11ème token
        let token_11 = PushToken::try_new("push_token_11").unwrap();
        settings.add_push_token(token_11).unwrap();

        // 3. Assertions
        assert_eq!(settings.push_tokens().len(), 10);
        // Le premier inséré ("push_token_0") doit avoir disparu, le nouveau index 0 est "push_token_1"
        assert_eq!(settings.push_tokens()[0].as_str(), "push_token_1");
    }

    #[test]
    fn test_push_token_removal() {
        let mut settings = create_test_settings();
        let token = PushToken::try_new("token_to_delete_xyz").unwrap();

        settings.add_push_token(token.clone()).unwrap();
        let _ = settings.metadata_mut().pull_events();

        let changed = settings.remove_push_token(&token).unwrap();
        assert!(changed);
        assert_eq!(settings.push_tokens().len(), 0);
    }

    #[test]
    fn test_update_appearance_preferences_idempotency() {
        let mut settings = create_test_settings();
        
        // Données par défaut identiques
        let identical_appearance = AppearancePreferences::default();
        let _ = settings.metadata_mut().pull_events();

        let changed = settings
            .update_appearance_preferences(identical_appearance)
            .unwrap();

        assert!(!changed);
        assert_eq!(settings.metadata_mut().pull_events().len(), 0);
    }
}
