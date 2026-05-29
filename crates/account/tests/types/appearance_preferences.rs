
use account::types::{AppearancePreferences, ThemeMode};

#[test]
fn test_appearance_default_is_system() {
    let appearance = AppearancePreferences::builder().build();

    assert_eq!(appearance.theme(), ThemeMode::System);
    assert!(!appearance.high_contrast());
}

#[test]
fn test_theme_selection_cycle() {
    let themes = vec![ThemeMode::Light, ThemeMode::Dark, ThemeMode::System];

    for theme in themes {
        let appearance = AppearancePreferences::builder().with_theme(theme).build();
        assert_eq!(appearance.theme(), theme);
    }
}

#[test]
fn test_high_contrast_toggle() {
    let appearance = AppearancePreferences::builder()
        .with_high_contrast(true)
        .build();

    assert!(appearance.high_contrast());
}
