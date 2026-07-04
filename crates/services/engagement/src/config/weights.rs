use std::collections::HashMap;

use crate::domain::value_object::ReactionKind;
use crate::error::EngagementError;

/// Dynamic reaction weight matrix loaded from environment variables at startup.
///
/// Each reaction kind maps to a positive integer weight. Weights drive the
/// `total_weighted_score` computed in Redis via HINCRBY. Reconfiguring weights
/// requires restarting the service — no hot-reload to prevent mid-flight weight
/// divergence between the Lua script ARGV and the stored `weight` ledger field.
///
/// # Environment variables
///
/// | Variable                               | Default |
/// |----------------------------------------|---------|
/// | `ENGAGEMENT_REACTION_WEIGHT_HEART`     | `1`     |
/// | `ENGAGEMENT_REACTION_WEIGHT_FIRE`      | `2`     |
/// | `ENGAGEMENT_REACTION_WEIGHT_ROCKET`    | `5`     |
/// | `ENGAGEMENT_REACTION_WEIGHT_CLAP`      | `1`     |
/// | `ENGAGEMENT_REACTION_WEIGHT_SAD`       | `1`     |
pub struct ReactionWeightsConfig {
    weights: HashMap<ReactionKind, i64>,
}

impl ReactionWeightsConfig {
    /// Loads weights from environment variables, falling back to defaults.
    pub fn from_env() -> Result<Self, EngagementError> {
        let mut weights = HashMap::new();

        for (kind, env_var, default) in Self::spec() {
            let weight = match std::env::var(env_var) {
                Ok(val) => val.parse::<i64>().map_err(|_| EngagementError::InvalidReactionWeight {
                    kind: kind.as_redis_key().to_owned(),
                    weight: 0,
                })?,
                Err(_) => *default,
            };

            if weight <= 0 {
                return Err(EngagementError::InvalidReactionWeight {
                    kind:   kind.as_redis_key().to_owned(),
                    weight,
                });
            }

            weights.insert(*kind, weight);
        }

        Ok(Self { weights })
    }

    /// Returns the weight for `kind`, defaulting to `1` if not configured.
    pub fn weight_of(&self, kind: ReactionKind) -> i64 {
        self.weights.get(&kind).copied().unwrap_or(1)
    }

    fn spec() -> &'static [(ReactionKind, &'static str, i64)] {
        &[
            (ReactionKind::Heart,  "ENGAGEMENT_REACTION_WEIGHT_HEART",  1),
            (ReactionKind::Fire,   "ENGAGEMENT_REACTION_WEIGHT_FIRE",   2),
            (ReactionKind::Rocket, "ENGAGEMENT_REACTION_WEIGHT_ROCKET", 5),
            (ReactionKind::Clap,   "ENGAGEMENT_REACTION_WEIGHT_CLAP",   1),
            (ReactionKind::Sad,    "ENGAGEMENT_REACTION_WEIGHT_SAD",    1),
        ]
    }
}
