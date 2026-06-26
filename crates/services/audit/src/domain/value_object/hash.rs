use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A hex-encoded SHA-256 digest — a link in the ledger's hash chain, or a Merkle
/// checkpoint root. Opaque and comparable; the audit plane only ever recomputes
/// and compares these, never parses them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RecordHash(String);

impl RecordHash {
    /// The chain's genesis predecessor — the `prev_hash` of the first record in a
    /// partition. A fixed, all-zero digest so the first link is well-defined.
    pub fn genesis() -> Self {
        RecordHash("0".repeat(64))
    }

    /// SHA-256 over `bytes`, hex-encoded. The single hashing primitive for the
    /// whole tamper-evidence layer.
    pub fn digest(bytes: &[u8]) -> Self {
        let out = Sha256::digest(bytes);
        RecordHash(to_hex(&out))
    }

    /// Reconstruct from a stored hex string (the ledger boundary). No validation
    /// beyond non-emptiness is needed — a malformed stored hash simply fails the
    /// next chain comparison, which is the detection we want.
    pub fn from_hex(value: impl Into<String>) -> Self {
        RecordHash(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RecordHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A deterministic, length-prefixed byte buffer used to canonicalize a value
/// before hashing.
///
/// Canonicalization is the load-bearing detail of a hash chain: the same logical
/// content must produce the same bytes on every machine, compiler and year, or
/// the chain stops verifying. Two rules make that hold here: (1) every field is
/// written in a **fixed order**, and (2) every variable-length field is
/// **length-prefixed**, so `"ab" + "c"` can never collide with `"a" + "bc"`. We
/// avoid JSON precisely because object key ordering and float formatting are not
/// canonical.
#[derive(Debug, Default)]
pub struct CanonicalWriter {
    buf: Vec<u8>,
}

impl CanonicalWriter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Append a length-prefixed byte field.
    pub fn bytes(&mut self, value: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(&(value.len() as u64).to_le_bytes());
        self.buf.extend_from_slice(value);
        self
    }

    /// Append a length-prefixed UTF-8 field.
    pub fn str(&mut self, value: &str) -> &mut Self {
        self.bytes(value.as_bytes())
    }

    /// Append a fixed-width `u8` discriminant (enum tags).
    pub fn u8(&mut self, value: u8) -> &mut Self {
        self.buf.push(value);
        self
    }

    /// Append a fixed-width `u64` (sequence numbers, timestamps).
    pub fn u64(&mut self, value: u64) -> &mut Self {
        self.buf.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Append a signed 64-bit value (epoch milliseconds).
    pub fn i64(&mut self, value: i64) -> &mut Self {
        self.buf.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Finish and hash the accumulated bytes.
    pub fn finish(&self) -> RecordHash {
        RecordHash::digest(&self.buf)
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_matches_known_vector() {
        // Pins the primitive to the standard SHA-256, hex-encoded, so a switch of
        // hashing library can never silently change the chain.
        let h = RecordHash::digest(b"audit");
        assert_eq!(
            h.as_str(),
            "b81f37a043a6f767e7c94d105f4bd31282f3ecc20680bb9d09bd93461cf4c863"
        );
    }

    #[test]
    fn genesis_is_64_zeros() {
        assert_eq!(RecordHash::genesis().as_str(), "0".repeat(64));
    }

    #[test]
    fn length_prefixing_prevents_concatenation_collisions() {
        // "ab"+"c" must not equal "a"+"bc".
        let mut a = CanonicalWriter::new();
        a.str("ab").str("c");
        let mut b = CanonicalWriter::new();
        b.str("a").str("bc");
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn same_input_same_hash() {
        let mut a = CanonicalWriter::new();
        a.str("evt").u8(3).u64(7);
        let mut b = CanonicalWriter::new();
        b.str("evt").u8(3).u64(7);
        assert_eq!(a.finish(), b.finish());
    }

    #[test]
    fn hex_is_lowercase_64_chars() {
        let h = RecordHash::digest(b"anything");
        assert_eq!(h.as_str().len(), 64);
        assert!(h.as_str().chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h.as_str().to_lowercase(), *h.as_str());
    }
}
