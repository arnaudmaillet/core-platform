#[cfg(test)]
mod tests {
    use crate::domain::preferences::models::NotificationPreferences;

    #[test]
    fn test_notifications_default_state() {
        let notifications = NotificationPreferences::builder().build();
        
        // Par défaut, on active souvent les canaux mais pas le marketing
        assert!(notifications.email_enabled());
        assert!(notifications.push_enabled());
        assert!(!notifications.marketing_opt_in());
        assert!(!notifications.security_alerts_only());
    }

    #[test]
    fn test_notifications_security_only_mode() {
        let notifications = NotificationPreferences::builder()
            .with_security_only(true)
            .with_marketing(true)
            .build();

        // On vérifie que les flags sont bien stockés
        assert!(notifications.security_alerts_only());
        assert!(notifications.marketing_opt_in());
        
        // On vérifie une méthode helper si tu en as une (ex: allows_marketing)
        // assert!(!notifications.allows_marketing()); 
    }

    #[test]
    fn test_disabling_all_channels() {
        let notifications = NotificationPreferences::builder()
            .with_email(false)
            .with_push(false)
            .build();

        assert!(!notifications.email_enabled());
        assert!(!notifications.push_enabled());
    }
}