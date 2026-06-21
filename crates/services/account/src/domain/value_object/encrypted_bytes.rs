use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// AES-256-GCM ciphertext blob.
///
/// Wraps raw ciphertext bytes that were encrypted by the platform's KMS or
/// Vault integration. The plaintext is never held in memory within this type.
///
/// # Secret protection
///
/// `Debug` is suppressed: the manual impl prints only the byte count, never
/// the actual content. `Display` is not implemented. This prevents the
/// ciphertext from appearing in logs or error messages.
///
/// # Serialisation
///
/// Serialised as a JSON array of raw bytes so the repository adapter can
/// pass the value directly to sqlx's `BYTEA` column type without additional
/// transformation. gRPC mappers must never include this field in API responses.
#[derive(Clone)]
pub struct EncryptedBytes(Vec<u8>);

impl EncryptedBytes {
    pub fn from_ciphertext(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for EncryptedBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EncryptedBytes([redacted; {} bytes])", self.0.len())
    }
}

impl Serialize for EncryptedBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for EncryptedBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BytesVisitor;

        impl<'de> de::Visitor<'de> for BytesVisitor {
            type Value = EncryptedBytes;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a byte array")
            }

            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                Ok(EncryptedBytes(v.to_vec()))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut bytes = Vec::new();
                while let Some(b) = seq.next_element::<u8>()? {
                    bytes.push(b);
                }
                Ok(EncryptedBytes(bytes))
            }
        }

        deserializer.deserialize_bytes(BytesVisitor)
    }
}
