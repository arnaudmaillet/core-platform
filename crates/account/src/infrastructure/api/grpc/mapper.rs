use crate::domain::account::entities::Account;
use crate::domain::value_objects::AccountState as DomainState;
use chrono::{DateTime, Utc};
use prost_types::Timestamp;
use shared_kernel::domain::{entities::Entity, events::AggregateRoot};
use shared_proto::account::v1::{
    AccountGovernance as ProtoGovernance, AccountIdentity as ProtoIdentity,
    AccountSettings as ProtoSettings, AccountState as ProtoState,
    account_settings::{
        Appearance as ProtoAppearence, Notifications as ProtoNotification, Privacy as ProtoPrivacy,
    },
};

/// Transforme l'agrégat Account (Domaine) en message AccountIdentity (Protobuf/gRPC)
pub fn map_account_to_identity_proto(account: Account) -> ProtoIdentity {
    let identity = account.identity();

    ProtoIdentity {
        account_id: identity.account_id().to_string(),
        sub_id: identity.sub_id().map(|id| id.to_string()),
        email: identity.email().map(|e| e.to_string()).unwrap_or_default(),
        email_verified: identity.is_email_verified(),

        phone_number: identity.phone_number().map(|p| p.to_string()),
        phone_verified: identity.is_phone_verified(),

        region_code: identity.region_code().to_string(),
        state: ProtoState::from(*identity.state()) as i32,

        birth_date: identity
            .birth_date()
            .map(|b| chrono_to_proto(b.to_utc_datetime())),
        locale: identity.locale().to_string(),
        created_at: Some(chrono_to_proto(identity.created_at())),
        updated_at: Some(chrono_to_proto(identity.updated_at())),
        aggregate_updated_at: Some(chrono_to_proto(identity.aggregate_updated_at())),
        last_active_at: identity.last_active_at().map(chrono_to_proto),

        version: account.version() as i64,
    }
}

pub fn map_account_to_settings_proto(account: Account) -> ProtoSettings {
    let settings = account.settings();

    ProtoSettings {
        account_id: account.id().to_string(),
        timezone: settings.timezone().to_string(),
        updated_at: Some(chrono_to_proto(settings.updated_at())),
        push_tokens: settings
            .push_tokens()
            .iter()
            .map(|t| t.to_string())
            .collect(),

        privacy: Some(ProtoPrivacy {
            profile_visible_to_public: settings.preferences().privacy().profile_visible_to_public(),
            show_last_active: settings.preferences().privacy().show_last_active(),
            allow_indexing: settings.preferences().privacy().allow_indexing(),
        }),

        notifications: Some(ProtoNotification {
            email_enabled: settings.preferences().notifications().email_enabled(),
            push_enabled: settings.preferences().notifications().push_enabled(),
            marketing_opt_in: settings.preferences().notifications().marketing_opt_in(),
            security_alerts_only: settings
                .preferences()
                .notifications()
                .security_alerts_only(),
        }),

        appearance: Some(ProtoAppearence {
            theme: settings.preferences().appearance().theme() as i32,
            high_contrast: settings.preferences().appearance().high_contrast(),
        }),
    }
}

pub fn map_account_to_governance_proto(account: Account) -> ProtoGovernance {
    let governance = account.governance();

    ProtoGovernance {
        account_id: account.id().to_string(),
        role: governance.role() as i32,
        trust_score: governance.trust_score().value(),
        is_shadowbanned: governance.is_shadowbanned(),
        is_beta_tester: governance.is_beta_tester(),
        last_moderation_at: governance.last_moderation_at().map(chrono_to_proto),
        moderation_notes: governance.moderation_notes().map(|s| s.to_string()),
        estimated_ip: governance.last_ip_addr().map(|ip| ip.to_string()),
        updated_at: Some(chrono_to_proto(governance.updated_at())),
    }
}

fn chrono_to_proto(dt: DateTime<Utc>) -> Timestamp {
    Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

impl From<DomainState> for ProtoState {
    fn from(state: DomainState) -> Self {
        match state {
            DomainState::Pending => Self::Pending,
            DomainState::Active => Self::Active,
            DomainState::Deactivated => Self::Deactivated,
            DomainState::Suspended => Self::Suspended,
            DomainState::Banned => Self::Banned,
        }
    }
}
