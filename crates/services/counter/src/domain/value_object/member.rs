use serde::{Deserialize, Serialize};

use crate::error::CounterError;

/// The actor/viewer id folded into a HyperLogLog for a cardinality metric (unique
/// viewers, reach).
///
/// Privacy note (the boundary): a `MemberId` exists only **transiently**, inside
/// an open aggregation window, to be `PFADD`-ed into a probabilistic estimator.
/// It is never persisted as identity and never leaves the service — the HLL keeps
/// no membership, only an estimate. This is what lets counter answer "how many
/// unique viewers?" without ever owning "who viewed?" (that edge state is not
/// counter's to hold).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MemberId(String);

impl MemberId {
    pub fn new(value: impl Into<String>) -> Result<Self, CounterError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(CounterError::InvalidIdentifier("member_id".to_owned()));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn accepts_non_empty() {
        assert_eq!(MemberId::new("viewer-1").unwrap().as_str(), "viewer-1");
    }

    #[test]
    fn rejects_blank() {
        let err = MemberId::new("").unwrap_err();
        assert_eq!(err.error_code(), "CTR-9002");
    }
}
