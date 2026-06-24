use super::exponential::{ExponentialBackoff, JitterKind};

/// Non-generic, deserializable description of a backoff strategy.
///
/// [`crate::retry::config::RetryConfig`] is generic over `B: BackoffStrategy` for
/// zero-cost dispatch — which means it can't be `Deserialize`d directly. `BackoffSpec`
/// is the *wire* counterpart: a flat, tagged enum the config layer reads from
/// `infrastructure.toml`, then [`resolve`](BackoffSpec::resolve)s into the concrete,
/// monomorphized strategy. Serialization logic stays out of the core trait boundary.
///
/// ```toml
/// backoff = { kind = "exponential", base_ms = 50, max_ms = 10_000, jitter = "full" }
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", rename_all = "snake_case"))]
pub enum BackoffSpec {
    /// Resolves into [`ExponentialBackoff`].
    Exponential {
        base_ms: u64,
        max_ms: u64,
        #[cfg_attr(feature = "serde", serde(default))]
        jitter: JitterKind,
    },
}

impl BackoffSpec {
    /// Lowers the wire description into the concrete, zero-cost strategy used on the hot path.
    pub fn resolve(self) -> ExponentialBackoff {
        match self {
            BackoffSpec::Exponential { base_ms, max_ms, jitter } => {
                ExponentialBackoff::new(base_ms, max_ms, jitter)
            }
        }
    }
}

impl Default for BackoffSpec {
    /// Mirrors [`ExponentialBackoff::default`]: 50ms base, 10s cap, full jitter.
    fn default() -> Self {
        BackoffSpec::Exponential {
            base_ms: 50,
            max_ms: 10_000,
            jitter: JitterKind::Full,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_into_concrete_strategy() {
        let backoff = BackoffSpec::Exponential {
            base_ms: 20,
            max_ms: 500,
            jitter: JitterKind::None,
        }
        .resolve();

        assert_eq!(backoff.base_ms, 20);
        assert_eq!(backoff.max_ms, 500);
        assert_eq!(backoff.jitter, JitterKind::None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn deserializes_tagged_form_with_default_jitter() {
        let spec: BackoffSpec =
            serde_json::from_str(r#"{ "kind": "exponential", "base_ms": 50, "max_ms": 10000 }"#)
                .unwrap();

        match spec {
            BackoffSpec::Exponential { base_ms, max_ms, jitter } => {
                assert_eq!((base_ms, max_ms), (50, 10_000));
                assert_eq!(jitter, JitterKind::Full); // serde(default)
            }
        }
    }
}
