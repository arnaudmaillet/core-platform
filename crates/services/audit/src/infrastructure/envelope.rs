//! Pure AES-256-GCM envelope-encryption primitives — the crypto behind
//! rationale-sealing, with no I/O so it is fully unit-testable. The key custody
//! (the per-subject DEK store + the KEK) lives in the adapter
//! ([`super::aes_gcm_cipher`]); this module only does the math.
//!
//! Scheme: a random 256-bit **data-encryption key (DEK)** encrypts the plaintext;
//! the DEK is itself encrypted ("wrapped") under a service **key-encryption key
//! (KEK)**. Storing only the wrapped DEK (in `subject_keys`) while keeping the KEK
//! out of the database means a DB operator alone cannot decrypt — and destroying
//! the wrapped DEK row (crypto-shred) renders the data permanently unreadable.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use rand::TryRngCore;
use rand::rngs::OsRng;

use crate::error::AuditError;

/// AES-256-GCM key length.
pub const KEY_LEN: usize = 32;
/// AES-GCM nonce length (96-bit, the standard).
pub const NONCE_LEN: usize = 12;
/// Algorithm tag recorded on the envelope (for crypto-agility on read).
pub const ALGORITHM: &str = "AES-256-GCM";

/// A freshly-encrypted blob: ciphertext (incl. the GCM tag) + its nonce.
pub struct Sealed {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

/// Mint a cryptographically-random 256-bit key (a DEK), via the OS CSPRNG.
pub fn random_key() -> Result<[u8; KEY_LEN], AuditError> {
    let mut key = [0u8; KEY_LEN];
    OsRng
        .try_fill_bytes(&mut key)
        .map_err(|e| AuditError::DomainViolation {
            field: "dek".to_owned(),
            message: format!("CSPRNG failure: {e}"),
        })?;
    Ok(key)
}

/// Encrypt `plaintext` under `key` with a fresh random nonce.
pub fn seal(key: &[u8; KEY_LEN], plaintext: &[u8]) -> Result<Sealed, AuditError> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|e| crypto_err(format!("CSPRNG failure: {e}")))?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| crypto_err("AEAD seal failed".to_owned()))?;

    Ok(Sealed {
        ciphertext,
        nonce: nonce_bytes.to_vec(),
    })
}

/// Decrypt under `key`. A wrong key, altered ciphertext, or wrong nonce fails the
/// GCM tag check — surfaced as the *expected* post-erasure state when the DEK is
/// gone, `AUD-5004`.
pub fn open(key: &[u8; KEY_LEN], ciphertext: &[u8], nonce: &[u8]) -> Result<Vec<u8>, AuditError> {
    if nonce.len() != NONCE_LEN {
        return Err(AuditError::PiiEnvelopeUndecryptable);
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| AuditError::PiiEnvelopeUndecryptable)
}

fn crypto_err(message: String) -> AuditError {
    AuditError::DomainViolation {
        field: "pii".to_owned(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn seal_open_roundtrips() {
        let key = random_key().unwrap();
        let sealed = seal(&key, b"violates harassment policy 3.2").unwrap();
        let plain = open(&key, &sealed.ciphertext, &sealed.nonce).unwrap();
        assert_eq!(plain, b"violates harassment policy 3.2");
        assert_eq!(sealed.nonce.len(), NONCE_LEN);
        assert_ne!(sealed.ciphertext, b"violates harassment policy 3.2");
    }

    #[test]
    fn nonces_differ_per_seal() {
        let key = random_key().unwrap();
        let a = seal(&key, b"x").unwrap();
        let b = seal(&key, b"x").unwrap();
        assert_ne!(a.nonce, b.nonce, "each seal must use a fresh nonce");
    }

    #[test]
    fn wrong_key_cannot_decrypt() {
        let key = random_key().unwrap();
        let other = random_key().unwrap();
        let sealed = seal(&key, b"secret").unwrap();
        // A destroyed DEK is modelled as "no longer have the right key".
        let err = open(&other, &sealed.ciphertext, &sealed.nonce).unwrap_err();
        assert_eq!(err.error_code(), "AUD-5004");
    }

    #[test]
    fn tampered_ciphertext_fails_the_tag() {
        let key = random_key().unwrap();
        let mut sealed = seal(&key, b"secret").unwrap();
        sealed.ciphertext[0] ^= 0xff;
        assert!(open(&key, &sealed.ciphertext, &sealed.nonce).is_err());
    }
}
