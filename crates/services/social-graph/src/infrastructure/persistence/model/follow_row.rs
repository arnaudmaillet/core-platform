use scylla::value::CqlTimestamp;
use scylla::DeserializeRow;
use uuid::Uuid;

/// Positional deserialization target for rows from the `followers` and `following` tables.
///
/// SELECT must always emit `(profile_id_column, followed_at)` in this order.
/// `enforce_order` + `skip_name_checks` matches fields by POSITION only, so the
/// same struct serves both tables regardless of whether the first column is named
/// `follower_id` or `followee_id`. Without `skip_name_checks`, `enforce_order`
/// still verifies names and rejects both queries (`profile_id` != the CQL column
/// name) — the timeline feed rebuild failed 100% on this until the staging soak
/// exposed it.
#[derive(DeserializeRow)]
#[scylla(flavor = "enforce_order", skip_name_checks)]
pub struct FollowRow {
    pub profile_id:  Uuid,
    pub followed_at: CqlTimestamp,
}
