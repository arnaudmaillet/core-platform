use account_api::account_service_client::AccountServiceClient;
use account_api::GetAccountByIdRequest;
use async_trait::async_trait;
use tonic::transport::Channel;
use tonic::Code;

use crate::application::port::AccountDirectory;
use crate::domain::value_object::ActorId;
use crate::error::ModerationError;

/// gRPC implementation of [`AccountDirectory`], backed by the `account` service.
/// The tonic client is cheaply cloneable (the `Channel` is `Arc`-backed), so each
/// call clones it to satisfy the `&self` port signature.
#[derive(Clone)]
pub struct GrpcAccountDirectory {
    client: AccountServiceClient<Channel>,
}

impl GrpcAccountDirectory {
    pub fn new(channel: Channel) -> Self {
        Self { client: AccountServiceClient::new(channel) }
    }
}

#[async_trait]
impl AccountDirectory for GrpcAccountDirectory {
    async fn actor_exists(&self, actor_id: &ActorId) -> Result<bool, ModerationError> {
        let mut client = self.client.clone();
        match client
            .get_account_by_id(GetAccountByIdRequest { account_id: actor_id.as_str() })
            .await
        {
            Ok(_) => Ok(true),
            Err(status) if status.code() == Code::NotFound => Ok(false),
            Err(_) => Err(ModerationError::AccountDirectoryUnavailable),
        }
    }
}
