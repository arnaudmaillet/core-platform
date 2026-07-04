use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::MediaError;

/// The asset lifecycle state. The legal transitions are:
///
/// ```text
///   Pending ─▶ Uploaded ─▶ Processing ─▶ Ready
///                 │             │
///                 └──────┬──────┘
///                        ▼
///                      Failed
///   {any live state} ─▶ Quarantined ─▶ {restored to prior}
///   {any non-held state} ─▶ Deleted
/// ```
///
/// `Quarantine` and `Delete` are cross-cutting and handled by the aggregate (they
/// can apply from several states), so they are intentionally NOT in
/// [`can_transition_to`]; that method governs only the forward processing path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetState {
    /// Reserved at ticket time; awaiting the client's direct-to-store upload.
    Pending,
    /// Bytes landed and finalize accepted; validated, pre-derivation.
    Uploaded,
    /// The transformation pipeline is running.
    Processing,
    /// All renditions available; deliverable.
    Ready,
    /// Terminal processing failure (corrupt / unsupported / pipeline error).
    Failed,
    /// Delivery revoked by a moderation takedown / compliance hold (reversible).
    Quarantined,
    /// Hard-deleted (bytes purged). Terminal.
    Deleted,
}

impl AssetState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Uploaded => "uploaded",
            Self::Processing => "processing",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Quarantined => "quarantined",
            Self::Deleted => "deleted",
        }
    }

    /// The forward processing path only (Quarantine/Delete are handled separately).
    pub fn can_transition_to(&self, next: AssetState) -> bool {
        use AssetState::*;
        matches!(
            (self, next),
            (Pending, Uploaded)
                | (Uploaded, Processing)
                | (Uploaded, Failed)
                | (Processing, Ready)
                | (Processing, Failed)
        )
    }

    /// Deleted is the only fully terminal state; Failed/Quarantined are still
    /// actionable (reprocess / restore).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Deleted)
    }

    /// Whether an asset in this state may have a delivery URL resolved for it.
    /// Only `Ready` is deliverable; everything else resolves to a placeholder
    /// (Plane C fails open).
    pub fn is_deliverable(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

impl fmt::Display for AssetState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for AssetState {
    type Error = MediaError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "pending" => Ok(Self::Pending),
            "uploaded" => Ok(Self::Uploaded),
            "processing" => Ok(Self::Processing),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            "quarantined" => Ok(Self::Quarantined),
            "deleted" => Ok(Self::Deleted),
            other => Err(MediaError::DomainViolation {
                field: "asset_state".into(),
                message: format!("unknown asset state: '{other}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_transitions_are_allowed() {
        assert!(AssetState::Pending.can_transition_to(AssetState::Uploaded));
        assert!(AssetState::Uploaded.can_transition_to(AssetState::Processing));
        assert!(AssetState::Processing.can_transition_to(AssetState::Ready));
    }

    #[test]
    fn illegal_jumps_are_rejected() {
        assert!(!AssetState::Pending.can_transition_to(AssetState::Ready));
        assert!(!AssetState::Ready.can_transition_to(AssetState::Processing));
        assert!(!AssetState::Deleted.can_transition_to(AssetState::Ready));
    }

    #[test]
    fn only_ready_is_deliverable_only_deleted_is_terminal() {
        assert!(AssetState::Ready.is_deliverable());
        for s in [
            AssetState::Pending,
            AssetState::Uploaded,
            AssetState::Processing,
            AssetState::Failed,
            AssetState::Quarantined,
        ] {
            assert!(!s.is_deliverable());
        }
        assert!(AssetState::Deleted.is_terminal());
        assert!(!AssetState::Quarantined.is_terminal());
    }

    #[test]
    fn string_round_trip() {
        for s in [
            AssetState::Pending,
            AssetState::Uploaded,
            AssetState::Processing,
            AssetState::Ready,
            AssetState::Failed,
            AssetState::Quarantined,
            AssetState::Deleted,
        ] {
            assert_eq!(AssetState::try_from(s.as_str()).unwrap(), s);
        }
        assert!(AssetState::try_from("bogus").is_err());
    }
}
