use std::collections::HashMap;

use base64::Engine as _;
use jsonwebtoken::{DecodingKey, EncodingKey};
// rsa 0.9 requires rand_core 0.6; use its re-exported OsRng to avoid the
// version split caused by the workspace's rand crate (rand_core 0.9).
use rsa::rand_core::OsRng;
use rsa::traits::PublicKeyParts as _;
use rsa::{pkcs1::EncodeRsaPrivateKey, RsaPrivateKey};

/// In-memory RSA-2048 key pair produced at test-start, with no disk I/O and
/// no external process dependencies.
///
/// Each call to [`TestKeyPair::generate`] produces a fresh, unique key pair
/// so test runs are isolated from one another even when run in parallel.
pub struct TestKeyPair {
    pub kid: String,
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
}

impl TestKeyPair {
    /// Generates a fresh RSA-2048 key pair.
    ///
    /// Using 2048 bits keeps test execution fast while still exercising the
    /// real RS256 code path. Production SHOULD use 3072 or 4096 bits.
    ///
    /// # Panics
    ///
    /// Panics if key generation fails — considered fatal in a test environment.
    pub fn generate() -> Self {
        let private_key = RsaPrivateKey::new(&mut OsRng, 2048)
            .expect("RSA-2048 key generation failed");

        let pem = private_key
            .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
            .expect("failed to encode private key as PKCS#1 PEM");

        let encoding_key = EncodingKey::from_rsa_pem(pem.as_bytes())
            .expect("failed to build EncodingKey from PKCS#1 PEM");

        let n_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(private_key.n().to_bytes_be());
        let e_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(private_key.e().to_bytes_be());

        let decoding_key = DecodingKey::from_rsa_components(&n_b64, &e_b64)
            .expect("failed to build DecodingKey from RSA components");

        // Use now_v7 — the workspace uuid crate only enables the v7 feature.
        let kid = uuid::Uuid::now_v7().to_string();

        Self {
            kid,
            encoding_key,
            decoding_key,
        }
    }

    /// Returns a `kid → DecodingKey` map suitable for pre-seeding a
    /// [`JwksCache`] without performing any network fetch.
    ///
    /// Does not consume `self` — the `EncodingKey` remains available for
    /// subsequent token minting in the same test.
    ///
    /// [`JwksCache`]: auth_context::JwksCache
    pub fn as_cache_map(&self) -> HashMap<String, DecodingKey> {
        let mut map = HashMap::new();
        map.insert(self.kid.clone(), self.decoding_key.clone());
        map
    }
}
