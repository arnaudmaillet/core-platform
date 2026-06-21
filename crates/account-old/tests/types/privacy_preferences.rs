
use account_old::types::PrivacyPreferences;

#[test]
fn test_privacy_default_values() {
    let privacy = PrivacyPreferences::builder().build();

    // Vérifie les réglages par défaut (généralement permissifs au départ)
    assert!(privacy.profile_visible_to_public());
    assert!(privacy.show_last_active());
    assert!(privacy.allow_indexing());
}

#[test]
fn test_business_rule_private_profile_disables_indexing() {
    // RÈGLE : Si le profil n'est pas public, l'indexation DOIT être fausse
    let privacy = PrivacyPreferences::builder()
        .with_public_profile(false)
        .with_indexing(true) // On tente de forcer à true
        .build();

    assert!(!privacy.profile_visible_to_public());
    assert!(
        !privacy.allow_indexing(),
        "Indexing must be forced to false if profile is private"
    );
}

#[test]
fn test_privacy_full_customization() {
    let privacy = PrivacyPreferences::builder()
        .with_public_profile(true)
        .with_last_active(false)
        .with_indexing(true)
        .build();

    assert!(privacy.profile_visible_to_public());
    assert!(!privacy.show_last_active());
    assert!(privacy.allow_indexing());
}
