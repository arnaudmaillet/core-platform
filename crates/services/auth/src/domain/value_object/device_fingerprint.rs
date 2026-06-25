use serde::{Deserialize, Serialize};

/// Best-effort client/device metadata captured at login for session tracking
/// and anomaly signalling.
///
/// None of these fields are security-bearing on their own — they label sessions
/// in the device-management view and let a refresh optionally re-check that the
/// presenting device matches the one the session was bound to. All components
/// are optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceFingerprint {
    user_agent: Option<String>,
    ip_address: Option<String>,
    device_id: Option<String>,
}

impl DeviceFingerprint {
    pub fn new(
        user_agent: Option<String>,
        ip_address: Option<String>,
        device_id: Option<String>,
    ) -> Self {
        Self { user_agent, ip_address, device_id }
    }

    pub fn user_agent(&self) -> Option<&str> {
        self.user_agent.as_deref()
    }

    pub fn ip_address(&self) -> Option<&str> {
        self.ip_address.as_deref()
    }

    pub fn device_id(&self) -> Option<&str> {
        self.device_id.as_deref()
    }

    /// Whether `other` plausibly originates from the same device.
    ///
    /// Compares the stable `device_id` when both sides provide one; if either is
    /// absent the check is inconclusive and returns `true` (fail-open — device
    /// binding is a signal, not an authentication factor).
    pub fn same_device_as(&self, other: &DeviceFingerprint) -> bool {
        match (self.device_id(), other.device_id()) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_device(id: &str) -> DeviceFingerprint {
        DeviceFingerprint::new(None, None, Some(id.to_owned()))
    }

    #[test]
    fn same_device_when_ids_match() {
        assert!(with_device("d1").same_device_as(&with_device("d1")));
    }

    #[test]
    fn different_device_when_ids_differ() {
        assert!(!with_device("d1").same_device_as(&with_device("d2")));
    }

    #[test]
    fn inconclusive_when_id_missing_is_fail_open() {
        let none = DeviceFingerprint::default();
        assert!(with_device("d1").same_device_as(&none));
        assert!(none.same_device_as(&with_device("d1")));
    }
}
