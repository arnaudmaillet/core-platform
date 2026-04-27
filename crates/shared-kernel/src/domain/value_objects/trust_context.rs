// crates/shared-kernel/src/domain/value_objects/trust_context.rs

use std::fmt;

pub enum TrustContext {
    EmailVerified,
    PhoneVerified,
    SuspensionLifted,
    UnbanBonus,
    AccountBanned,
    ManualAdjustment,
    SystemAutomatic,
}

impl fmt::Display for TrustContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::EmailVerified => "Email verified",
            Self::PhoneVerified => "Phone verified",
            Self::SuspensionLifted => "Suspension lifted",
            Self::UnbanBonus => "Unban bonus",
            Self::AccountBanned => "Account banned",
            Self::ManualAdjustment => "Manual adjustment",
            Self::SystemAutomatic => "System automatic",
        };
        write!(f, "{}", s)
    }
}
