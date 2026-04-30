#[cfg(test)]
mod tests {
    use crate::domain::{
        account::entities::{AccountPreferences, AccountSettings},
        preferences::models::{AppearancePreferences, NotificationPreferences, PrivacyPreferences},
    };
    use chrono::Utc;
    use shared_kernel::{
        domain::value_objects::{AccountId, PushToken, RegionCode, Timezone},
        errors::Result,
    };

    fn create_test_settings() -> Result<AccountSettings> {
        let account_id = AccountId::new();
        let preferences = AccountPreferences::new(
            PrivacyPreferences::default(),
            NotificationPreferences::default(),
            AppearancePreferences::default(),
        );

        Ok(AccountSettings::restore(
            account_id,
            preferences,
            Timezone::try_new("UTC")?,
            vec![],
            Utc::now(),
        ))
    }

    #[test]
    fn test_timezone_update_logic_and_idempotency() -> Result<()> {
        let mut settings = create_test_settings()?;
        let region = RegionCode::try_new("eu")?;
        let new_tz = Timezone::try_new("Europe/Paris")?;

        // 1. Premier passage : mutation acceptée
        let changed = settings.apply_timezone_update(new_tz.clone(), &region)?;
        assert!(changed);
        assert_eq!(settings.timezone().as_str(), "Europe/Paris");

        // 2. Idempotence : même valeur
        let changed_again = settings.apply_timezone_update(new_tz, &region)?;
        assert!(!changed_again);

        Ok(())
    }

    #[test]
    fn test_push_token_fifo_rotation() -> Result<()> {
        let mut settings = create_test_settings()?;

        // 1. Remplissage jusqu'à la limite (10 tokens)
        for i in 0..10 {
            let token = PushToken::try_new(format!("push_token_{:02}", i))?;
            settings.apply_push_token_add(token);
        }
        assert_eq!(settings.push_tokens().len(), 10);

        // 2. Ajout du 11ème token : déclenche la rotation FIFO (le premier sort)
        let token_11 = PushToken::try_new("push_token_11")?;
        let changed = settings.apply_push_token_add(token_11);

        assert!(changed);
        assert_eq!(settings.push_tokens().len(), 10);
        // "push_token_00" doit avoir été supprimé, le nouveau premier est "push_token_01"
        assert_eq!(settings.push_tokens()[0].as_str(), "push_token_01");
        // Le dernier est bien le nouveau token
        assert_eq!(settings.push_tokens()[9].as_str(), "push_token_11");

        Ok(())
    }

    #[test]
    fn test_update_appearance_preferences_idempotency() -> Result<()> {
        let mut settings = create_test_settings()?;

        // Les préférences par défaut sont déjà chargées
        let identical_appearance = AppearancePreferences::default();

        // On tente de mettre à jour avec exactement la même chose
        let changed = settings.apply_appearance_update(identical_appearance);

        assert!(!changed);

        Ok(())
    }

    #[test]
    fn test_timezone_region_inconsistency() -> Result<()> {
        let mut settings = create_test_settings()?;
        let region_eu = RegionCode::try_new("eu")?;

        // Exemple d'une timezone incohérente avec la région (si ton VO implémente cette logique)
        let invalid_tz = Timezone::try_new("America/New_York")?;

        // On s'attend à ce que la règle de validation métier dans apply_timezone_update bloque cela
        let result = settings.apply_timezone_update(invalid_tz, &region_eu);
        assert!(result.is_err());

        Ok(())
    }
}
