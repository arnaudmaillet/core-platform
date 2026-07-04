//! A minimal **AWS Signature Version 4** signer for header-authenticated JSON
//! POSTs — just enough to call the KMS query API (`TrentService`).
//!
//! The fleet deliberately avoids the AWS SDK (the object store signs its own S3
//! URLs via `rusty-s3`); KMS, however, needs *header-based* SigV4 over a signed
//! request body, which `rusty-s3` does not expose. Rather than pull in the SDK we
//! sign here, the same way the rest of the platform signs its outbound requests:
//! explicitly, with no I/O, fully unit-testable against the published AWS SigV4
//! test-suite vectors.
//!
//! Scope is intentionally narrow: single-chunk (non-streaming) requests, an
//! already-sorted set of signed headers, SHA-256 payload hashing. HMAC-SHA256 is
//! implemented locally over the workspace `sha2` so no `hmac`/`hex`/`digest`
//! version juggling leaks in.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

const ALGORITHM: &str = "AWS4-HMAC-SHA256";
const TERMINATOR: &str = "aws4_request";
/// SHA-256 internal block size (HMAC key padding).
const HMAC_BLOCK: usize = 64;

/// Static AWS credentials + the region/service scope a signer is bound to.
#[derive(Debug, Clone)]
pub struct SigV4Credentials {
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub service: String,
}

/// One header that participates in the signature. `name` MUST be lowercase; the
/// caller passes these pre-sorted by name (the canonical-headers ordering rule).
pub struct SignedHeader<'a> {
    pub name: &'a str,
    pub value: &'a str,
}

/// The two headers the caller must attach to the outbound request for the
/// signature to validate: `Authorization` and `X-Amz-Date`.
pub struct SignedRequest {
    pub authorization: String,
    pub amz_date: String,
}

/// Compute the SigV4 `Authorization` header for a request.
///
/// `headers` must be lowercase-named and sorted by name; `host` is supplied as
/// one of them. `payload` is the exact request body bytes that will be sent.
pub fn sign(
    creds: &SigV4Credentials,
    method: &str,
    canonical_uri: &str,
    canonical_query: &str,
    headers: &[SignedHeader<'_>],
    payload: &[u8],
    now: DateTime<Utc>,
) -> SignedRequest {
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();

    let payload_hash = hex(&Sha256::digest(payload));

    let mut canonical_headers = String::new();
    let mut signed_headers = String::new();
    for (i, h) in headers.iter().enumerate() {
        canonical_headers.push_str(h.name);
        canonical_headers.push(':');
        canonical_headers.push_str(h.value.trim());
        canonical_headers.push('\n');
        if i > 0 {
            signed_headers.push(';');
        }
        signed_headers.push_str(h.name);
    }

    let canonical_request = format!(
        "{method}\n{canonical_uri}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );

    let scope = format!(
        "{date_stamp}/{}/{}/{TERMINATOR}",
        creds.region, creds.service
    );
    let string_to_sign = format!(
        "{ALGORITHM}\n{amz_date}\n{scope}\n{}",
        hex(&Sha256::digest(canonical_request.as_bytes()))
    );

    let signing_key = derive_signing_key(creds, &date_stamp);
    let signature = hex(&hmac_sha256(&signing_key, string_to_sign.as_bytes()));

    let authorization = format!(
        "{ALGORITHM} Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
        creds.access_key
    );

    SignedRequest {
        authorization,
        amz_date,
    }
}

/// The SigV4 four-step key derivation: `AWS4‖secret` → date → region → service →
/// `aws4_request`.
fn derive_signing_key(creds: &SigV4Credentials, date_stamp: &str) -> [u8; 32] {
    let k_date = hmac_sha256(
        format!("AWS4{}", creds.secret_key).as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, creds.region.as_bytes());
    let k_service = hmac_sha256(&k_region, creds.service.as_bytes());
    hmac_sha256(&k_service, TERMINATOR.as_bytes())
}

/// HMAC-SHA256, implemented over the workspace `sha2` so the audit crate carries
/// no `hmac`/`digest` version pin (the fleet has both `digest` 0.10 and 0.11 in
/// tree; mixing them through `hmac` is avoidable).
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut block = [0u8; HMAC_BLOCK];
    if key.len() > HMAC_BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; HMAC_BLOCK];
    let mut opad = [0x5cu8; HMAC_BLOCK];
    for i in 0..HMAC_BLOCK {
        ipad[i] ^= block[i];
        opad[i] ^= block[i];
    }

    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    outer.finalize().into()
}

/// Lowercase hex — SigV4 requires it for the payload hash and the signature.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    /// RFC 4231 HMAC-SHA256 test case 2 — pins the locally-implemented HMAC to the
    /// standard so a refactor can never silently corrupt every signature.
    #[test]
    fn hmac_matches_rfc4231_case2() {
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex(&mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    /// The canonical AWS SigV4 test-suite `get-vanilla` vector. If this passes, the
    /// canonical-request build, the credential scope, the key derivation and the
    /// final signature all match AWS byte-for-byte — so the live KMS calls will
    /// authenticate.
    #[test]
    fn get_vanilla_matches_aws_test_suite() {
        let creds = SigV4Credentials {
            access_key: "AKIDEXAMPLE".to_owned(),
            secret_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".to_owned(),
            region: "us-east-1".to_owned(),
            service: "service".to_owned(),
        };
        let now = Utc.with_ymd_and_hms(2015, 8, 30, 12, 36, 0).unwrap();
        let headers = [
            SignedHeader {
                name: "host",
                value: "example.amazonaws.com",
            },
            SignedHeader {
                name: "x-amz-date",
                value: "20150830T123600Z",
            },
        ];

        let signed = sign(&creds, "GET", "/", "", &headers, b"", now);

        assert_eq!(signed.amz_date, "20150830T123600Z");
        assert_eq!(
            signed.authorization,
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
             SignedHeaders=host;x-amz-date, \
             Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
        );
    }
}
