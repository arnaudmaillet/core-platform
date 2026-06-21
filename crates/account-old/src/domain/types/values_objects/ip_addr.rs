use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result};
use std::convert::TryFrom;
use std::fmt;
use std::net::IpAddr as StdIpAddr;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IpAddr(StdIpAddr); // Ton Value Object s'appelle IpAddr

impl IpAddr {
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();
        let cleaned = raw.trim();

        let parsed = StdIpAddr::from_str(cleaned).map_err(|e| {
            Error::validation("ip_addr", format!("Invalid IP address format: {}", e))
        })?;

        Ok(Self(parsed))
    }

    pub fn is_global(&self) -> bool {
        match self.0 {
            StdIpAddr::V4(addr) => {
                !addr.is_loopback() && !addr.is_private() && !addr.is_link_local()
            }
            StdIpAddr::V6(addr) => !addr.is_loopback() && !addr.is_unspecified(),
        }
    }

    pub fn to_db_string(&self) -> String {
        self.0.to_string()
    }

    pub fn to_std(&self) -> StdIpAddr {
        self.0
    }

    pub fn from_raw(ip: StdIpAddr) -> Self {
        Self(ip)
    }

    pub fn from_raw_str(ip_str: &str) -> Self {
        let parsed = StdIpAddr::from_str(ip_str).expect("CRITICAL: Database IP corruption");
        Self(parsed)
    }
}

impl fmt::Display for IpAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for IpAddr {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::try_new(value)
    }
}

impl TryFrom<String> for IpAddr {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::try_new(&value)
    }
}
