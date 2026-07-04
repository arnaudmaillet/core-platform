use chrono::{DateTime, Utc};

use crate::domain::value_object::{Aggregation, EntityRef, MemberId, Metric};
use crate::error::CounterError;

/// The single inbound atom the aggregator folds — the distilled, validated form
/// of one engagement.
///
/// Every wire event the service consumes (a view, an impression, a like/unlike, a
/// follow/unfollow) is decoded (Phase 4) into one or more `Observation`s, so the
/// fold never sees a topic, a schema, or a byte. One wire event may yield several
/// observations: a view event becomes both a `View` (`+1`, sum) **and** a
/// `UniqueViewer` (member-carrying, cardinality).
///
/// The shape is uniform on purpose:
/// * for [`Aggregation::Sum`] metrics, `amount` is the signed delta (`+1` like,
///   `-1` unlike, `+1/-1` follow) and `unique_member` is ignored;
/// * for [`Aggregation::Cardinality`] metrics, `unique_member` is the actor folded
///   into the HyperLogLog and `amount` is ignored.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub entity: EntityRef,
    pub metric: Metric,
    pub amount: i64,
    pub unique_member: Option<MemberId>,
    pub occurred_at: DateTime<Utc>,
}

impl Observation {
    /// A signed additive observation for a [`Aggregation::Sum`] metric. Rejects a
    /// cardinality metric (which needs a member, not an amount) as `CTR-2002
    /// InvalidDelta`.
    pub fn sum(
        entity: EntityRef,
        metric: Metric,
        amount: i64,
        occurred_at: DateTime<Utc>,
    ) -> Result<Self, CounterError> {
        if metric.aggregation() != Aggregation::Sum {
            return Err(CounterError::InvalidDelta {
                reason: format!("metric '{}' is not a sum metric", metric.as_str()),
            });
        }
        Ok(Self {
            entity,
            metric,
            amount,
            unique_member: None,
            occurred_at,
        })
    }

    /// A distinct-member observation for a [`Aggregation::Cardinality`] metric.
    /// Rejects a sum metric as `CTR-2002 InvalidDelta`.
    pub fn unique(
        entity: EntityRef,
        metric: Metric,
        member: MemberId,
        occurred_at: DateTime<Utc>,
    ) -> Result<Self, CounterError> {
        if metric.aggregation() != Aggregation::Cardinality {
            return Err(CounterError::InvalidDelta {
                reason: format!("metric '{}' is not a cardinality metric", metric.as_str()),
            });
        }
        Ok(Self {
            entity,
            metric,
            amount: 0,
            unique_member: Some(member),
            occurred_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use error::AppError;

    use super::*;
    use crate::domain::value_object::{EntityId, EntityKind};

    fn post() -> EntityRef {
        EntityRef::new(EntityKind::Post, EntityId::new("p1").unwrap())
    }

    fn t() -> DateTime<Utc> {
        Utc.timestamp_millis_opt(1_000).unwrap()
    }

    #[test]
    fn sum_observation_rejects_cardinality_metric() {
        let err = Observation::sum(post(), Metric::UniqueViewer, 1, t()).unwrap_err();
        assert_eq!(err.error_code(), "CTR-2002");
    }

    #[test]
    fn unique_observation_rejects_sum_metric() {
        let m = MemberId::new("v1").unwrap();
        let err = Observation::unique(post(), Metric::View, m, t()).unwrap_err();
        assert_eq!(err.error_code(), "CTR-2002");
    }

    #[test]
    fn signed_amounts_are_preserved() {
        let unlike = Observation::sum(post(), Metric::Like, -1, t()).unwrap();
        assert_eq!(unlike.amount, -1);
        assert!(unlike.unique_member.is_none());
    }
}
