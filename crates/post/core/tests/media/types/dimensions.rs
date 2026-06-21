#[cfg(test)]
mod tests {
    use post_older::{AspectRatio, Height, MAX_RESOLUTION, MIN_RESOLUTION, Width};

    #[test]
    fn test_width_validation() {
        // Succès
        assert!(Width::try_new(1080).is_ok());

        // Limites
        assert!(Width::try_new(MIN_RESOLUTION).is_ok());
        assert!(Width::try_new(MAX_RESOLUTION).is_ok());

        // Échecs
        assert!(Width::try_new(MIN_RESOLUTION - 1).is_err());
        assert!(Width::try_new(MAX_RESOLUTION + 1).is_err());
    }

    #[test]
    fn test_height_validation() {
        assert!(Height::try_new(1920).is_ok());
        assert!(Height::try_new(MIN_RESOLUTION - 1).is_err());
        assert!(Height::try_new(MAX_RESOLUTION + 1).is_err());
    }

    #[test]
    fn test_conversions_from_i32() {
        let w = Width::try_from(1080i32).unwrap();
        assert_eq!(w.value(), 1080);

        let w_neg = Width::try_from(-10i32);
        assert!(w_neg.is_err());
    }

    #[test]
    fn test_aspect_ratio_calculation() {
        // Vertical (9:16)
        let v_width = Width::from_raw(1080);
        let v_height = Height::from_raw(1920);
        assert_eq!(
            AspectRatio::from_dimensions(v_width, v_height),
            AspectRatio::Vertical
        );

        // Horizontal (16:9)
        let h_width = Width::from_raw(1920);
        let h_height = Height::from_raw(1080);
        assert_eq!(
            AspectRatio::from_dimensions(h_width, h_height),
            AspectRatio::Horizontal
        );

        // Square (1:1 - avec une marge de tolérance de 0.95 à 1.05)
        let s_width = Width::from_raw(1000);
        let s_height = Height::from_raw(1000);
        assert_eq!(
            AspectRatio::from_dimensions(s_width, s_height),
            AspectRatio::Square
        );

        // Square proche (ex: 1020x1000)
        let s_near_width = Width::from_raw(1020);
        let s_near_height = Height::from_raw(1000);
        assert_eq!(
            AspectRatio::from_dimensions(s_near_width, s_near_height),
            AspectRatio::Square
        );
    }
}
