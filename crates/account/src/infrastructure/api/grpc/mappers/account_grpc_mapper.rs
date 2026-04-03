use crate::domain::account::entities::Account;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::infrastructure::grpc::ChronoTimestampExt;
use shared_proto::account::v1::Account as ProtoAccount;
use shared_proto::account::v1::AccountState as ProtoState;

impl From<Account> for ProtoAccount {
    fn from(a: Account) -> Self {
        Self {
            id: a.id().to_string(),
            region_code: a.region_code().to_string(),
            external_id: a.external_id().to_string(),
            email: a.email().to_string(),
            email_verified: a.is_email_verified(),
            phone_number: a.phone_number().map(|p| p.to_string()),
            phone_verified: a.is_phone_verified(),
            state: ProtoState::from(*a.state()) as i32,
            version: a.version_i64().unwrap_or(0),
            created_at: Some(a.created_at().to_proto()),
            updated_at: Some(a.updated_at().to_proto()),
            last_active_at: a.last_active_at().map(|dt| dt.to_proto()),
            birth_date: a
                .birth_date()
                .as_ref()
                .map(|d| d.to_utc_datetime().to_proto()),
        }
    }
}
