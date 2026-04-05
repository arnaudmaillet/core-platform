use serde::{Deserialize, Serialize};
use shared_kernel::errors::{DomainError, Result};
use std::fmt;
use std::net::IpAddr as StdIpAddr;
use std::str::FromStr;
use std::convert::TryFrom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IpAddr(StdIpAddr); // Ton Value Object s'appelle IpAddr

impl IpAddr {
    pub fn try_new(ip_str: &str) -> Result<Self> {
        StdIpAddr::from_str(ip_str)
            .map(IpAddr)
            .map_err(|_| DomainError::Validation {
                field: "ip_address",
                reason: format!("L'adresse IP '{}' est mal formée", ip_str),
            })
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
        let parsed = StdIpAddr::from_str(ip_str)
            .expect("CRITICAL: Database IP corruption");
        Self(parsed)
    }
}

impl fmt::Display for IpAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for IpAddr {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self> {
        Self::try_new(value)
    }
}

impl TryFrom<String> for IpAddr {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self> {
        Self::try_new(&value)
    }
}