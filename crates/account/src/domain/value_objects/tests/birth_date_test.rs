
#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use super::*;
    use chrono::{Datelike, Duration, NaiveDate, Utc};
    use shared_kernel::errors::DomainError;
    use crate::domain::value_objects::BirthDate;

    /// Helper pour obtenir la date du jour (Naive)
    fn today() -> NaiveDate {
        Utc::now().date_naive()
    }

    #[test]
    fn test_birth_date_happy_path() {
        // Un utilisateur de 25 ans
        let date = today().with_year(today().year() - 25).unwrap();
        let birth_date = BirthDate::try_new(date);

        assert!(birth_date.is_ok());
        assert_eq!(birth_date.unwrap().value(), date);
    }

    #[test]
    fn test_birth_date_too_young() {
        // 1 jour avant ses 13 ans
        let date = today() - Duration::days(365 * BirthDate::MIN_AGE as i64 - 1);
        let result = BirthDate::try_new(date);

        assert!(matches!(result, Err(DomainError::Validation { ref field, .. }) if field.to_string() == "birth_date"));
        if let Err(DomainError::Validation { reason, .. }) = result {
            assert!(reason.contains("at least 13 years old"));
        }
    }

    #[test]
    fn test_has_reached_age() {
        let birth = today().with_year(today().year() - 18).unwrap();
        let vo = BirthDate::from_raw(birth);

        assert!(vo.has_reached_age(18));
        assert!(vo.has_reached_age(13));
        assert!(!vo.has_reached_age(21));
    }

    #[test]
    fn test_birth_date_exactly_min_age() {
        // Pile 13 ans aujourd'hui
        let date = today().with_year(today().year() - BirthDate::MIN_AGE as i32).unwrap();
        assert!(BirthDate::try_new(date).is_ok());
    }

    #[test]
    fn test_birth_date_too_old() {
        // 126 ans
        let date = today().with_year(today().year() - (BirthDate::MAX_AGE + 1) as i32).unwrap();
        let result = BirthDate::try_new(date);

        assert!(result.is_err());
        if let Err(DomainError::Validation { reason, .. }) = result {
            assert!(reason.contains("exceeds biological limits"));
        }
    }

    #[test]
    fn test_birth_date_in_future() {
        let date = today() + Duration::days(1);
        let result = BirthDate::try_new(date);

        assert!(result.is_err());
        if let Err(DomainError::Validation { reason, .. }) = result {
            assert_eq!(reason, "Birth date cannot be in the future");
        }
    }

    #[test]
    fn test_birth_date_from_raw_skips_validation() {
        // On simule une date invalide venant de la DB (ex: règle métier qui a changé)
        // from_raw ne doit jamais paniquer ni échouer
        let future_date = today() + Duration::days(100);
        let birth_date = BirthDate::from_raw(future_date);

        assert_eq!(birth_date.value(), future_date);
    }

    #[test]
    fn test_age_calculation_logic() {
        let birth = NaiveDate::from_ymd_opt(2000, 10, 15).expect("Invalid date");
        let vo = BirthDate::from_raw(birth);

        // 1. La veille (doit avoir 9 ans)
        assert_eq!(vo.age_at(NaiveDate::from_ymd_opt(2010, 10, 14).unwrap()), 9);

        // 2. Le jour même (doit avoir 10 ans)
        assert_eq!(vo.age_at(NaiveDate::from_ymd_opt(2010, 10, 15).unwrap()), 10);

        // 3. Le lendemain (doit avoir 10 ans)
        assert_eq!(vo.age_at(NaiveDate::from_ymd_opt(2010, 10, 16).unwrap()), 10);
    }

    #[test]
    fn test_from_str_parsing() {
        // Format valide
        let valid = BirthDate::from_str("1990-01-01");
        assert!(valid.is_ok());
        assert_eq!(valid.unwrap().value().year(), 1990);

        // Format invalide
        let invalid = BirthDate::from_str("01/01/1990");
        assert!(invalid.is_err());

        // Date inexistante (31 février)
        let impossible = BirthDate::from_str("1990-02-31");
        assert!(impossible.is_err());
    }

    #[test]
    fn test_serialization_cycle() {
        let date = NaiveDate::from_ymd_opt(1995, 5, 20).unwrap();
        let vo = BirthDate::try_new(date).unwrap();

        let serialized = serde_json::to_string(&vo).unwrap();
        // Vérifie que ça sérialise comme une NaiveDate standard
        assert_eq!(serialized, "\"1995-05-20\"");

        let deserialized: BirthDate = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, vo);
    }
}