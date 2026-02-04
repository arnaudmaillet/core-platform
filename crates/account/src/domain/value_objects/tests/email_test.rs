use unicode_normalization::UnicodeNormalization;
use crate::domain::value_objects::email::Email;
use shared_kernel::errors::DomainError;

#[test]
fn test_email_happy_path() {
    let valid_emails = vec![
        "user@example.com",
        "firstname.lastname@domain.co.uk",
        "user+tag@gmail.com",
        "1234567890@example.com",
        "email@example-one.com",
        "_______@example.com",
        "email@example.name",
        "email@example.museum",
        "email@example.ad",
    ];

    for addr in valid_emails {
        let result = Email::try_new(addr);
        assert!(result.is_ok(), "Should be valid: {}", addr);
        assert_eq!(result.unwrap().as_str(), addr);
    }
}

#[test]
fn test_email_normalization() {
    // 1. L'accent est APRÈS le 'e' -> "té"
    let decomposed = "te\u{0301}st@domain.com";
    let email = Email::try_new(decomposed).expect("Should normalize NFD to NFC");

    // 2. On s'attend donc à "tést@domain.com" (le é est au milieu)
    let expected_str = "tést@domain.com";

    assert_eq!(email.as_str(), expected_str);

    // 3. Calcul de la taille :
    // t(1) + é(2) + s(1) + t(1) + @(1) + domain.com(10) = 16 octets
    assert_eq!(email.as_str().len(), 16);
}


#[test]
fn test_email_case_and_trim() {
    let raw = "  USER@Example.COM  ";
    let email = Email::try_new(raw).unwrap();
    assert_eq!(email.as_str(), "user@example.com");
}


#[test]
fn test_email_invalid_formats() {
    let invalid_emails = vec![
        ".email@example.com",       // Point au début
        "email.@example.com",       // Point à la fin de la partie locale
        "email..email@example.com", // Double point
        "@example.com",             // Pas de local part
        "email@example",            // Pas de TLD
        "email@.example.com",       // Domaine commence par un point
    ];

    for addr in invalid_emails {
        let result = Email::try_new(addr);
        assert!(result.is_err(), "L'adresse suivante devrait être REJETÉE : {}", addr);
    }
}

#[test]
fn test_email_length_constraints() {
    // Trop court
    assert!(Email::try_new("").is_err());

    // Trop long (> 254)
    let long_local = "a".repeat(64);
    let long_domain = "b".repeat(190);
    let too_long = format!("{}@{}.com", long_local, long_domain);

    let result = Email::try_new(too_long);
    assert!(result.is_err());
    if let Err(DomainError::Validation { reason, .. }) = result {
        assert!(reason.contains("length must be between"));
    }
}

#[test]
fn test_email_domain_extraction() {
    let email = Email::try_new("user@sub.example.com").unwrap();
    assert_eq!(email.domain(), "sub.example.com");

    let email_no_domain = Email::from_raw("invalid-email");
    assert_eq!(email_no_domain.domain(), "invalid-email"); // split last sur un string sans @
}

#[test]
fn test_email_hashing_consistency() {
    let addr = "user@Example.com";
    let email1 = Email::try_new(addr).unwrap();
    let email2 = Email::try_new("user@example.com").unwrap();

    // La normalisation doit garantir le même hash
    assert_eq!(email1.hash_value(), email2.hash_value());
    assert_ne!(email1.hash_value(), 0);
}

#[test]
fn test_email_from_raw_skips_validation() {
    // Scénario : Donnée historique corrompue en DB
    let broken = "not-an-email";
    let email = Email::from_raw(broken);
    assert_eq!(email.as_str(), broken);
    // Mais si on valide manuellement, ça échoue
    assert!(shared_kernel::domain::value_objects::ValueObject::validate(&email).is_err());
}