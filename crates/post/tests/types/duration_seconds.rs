#[cfg(test)]
mod tests {
    use post::types::DurationSeconds;

    #[test]
    fn test_duration_validation_success() {
        assert!(DurationSeconds::try_new(1).is_ok());
        assert!(DurationSeconds::try_new(3600).is_ok());
        assert!(DurationSeconds::try_new(180).is_ok());
    }

    #[test]
    fn test_duration_validation_bounds() {
        assert!(DurationSeconds::try_new(0).is_ok());
        assert!(DurationSeconds::try_new(3601).is_err());
    }

    #[test]
    fn test_conversions_i32() {
        // Valide
        let d = DurationSeconds::try_from(45i32).unwrap();
        assert_eq!(d.value(), 45);

        // Négatif
        assert!(DurationSeconds::try_from(-1i32).is_err());
    }

    #[test]
    fn test_short_format_helper() {
        let short = DurationSeconds::from_raw(60);
        let long = DurationSeconds::from_raw(61);

        assert!(short.is_short_format());
        assert!(!long.is_short_format());
    }

    #[test]
    fn test_timestamp_formatting() {
        // 0 seconde
        assert_eq!(DurationSeconds::from_raw(0).to_timestamp_string(), "00:00");

        // Moins d'une minute
        assert_eq!(DurationSeconds::from_raw(45).to_timestamp_string(), "00:45");

        // Plus d'une minute
        assert_eq!(
            DurationSeconds::from_raw(125).to_timestamp_string(),
            "02:05"
        );

        // Exactement une heure
        assert_eq!(
            DurationSeconds::from_raw(3600).to_timestamp_string(),
            "60:00"
        );
    }

    #[test]
    fn test_display_trait() {
        let d = DurationSeconds::from_raw(120);
        assert_eq!(format!("{}", d), "120s");
    }
}
