// crates/account/src/infrastructure/api/grpc/mappers/state_grpc_mapper.rs


use shared_proto::account::v1::AccountState as ProtoState;
use crate::domain::value_objects::AccountState as DomainState;

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