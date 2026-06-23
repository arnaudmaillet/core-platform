use async_trait::async_trait;
use fred::interfaces::LuaInterface;
use redis_storage::RedisClient;

use crate::application::port::ReceiptStore;
use crate::domain::value_object::{ConversationId, MessageId, ProfileId};
use crate::error::ChatError;
use crate::infrastructure::cache::keys::receipts_key;
use crate::infrastructure::cache::redis_err;

/// Sets one member's read horizon. KEYS[1]=receipts ARGV[1]=member_id ARGV[2]=message_id
const HSET_FIELD: &str = r#"
redis.call('HSET', KEYS[1], ARGV[1], ARGV[2])
return 1
"#;

/// Returns the full horizon map as a flat [field, value, ...] array.
const HGETALL: &str = r#"
return redis.call('HGETALL', KEYS[1])
"#;

pub struct RedisReceiptStore {
    client: RedisClient,
}

impl RedisReceiptStore {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ReceiptStore for RedisReceiptStore {
    async fn set(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
        last_read:       MessageId,
    ) -> Result<(), ChatError> {
        let _: i64 = self
            .client
            .inner
            .eval(
                HSET_FIELD,
                vec![receipts_key(conversation_id)],
                vec![member_id.as_str(), last_read.as_str()],
            )
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    async fn all(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Vec<(ProfileId, MessageId)>, ChatError> {
        let flat: Vec<String> = self
            .client
            .inner
            .eval(HGETALL, vec![receipts_key(conversation_id)], Vec::<String>::new())
            .await
            .map_err(redis_err)?;

        if !flat.len().is_multiple_of(2) {
            return Err(ChatError::DomainViolation {
                field:   "receipts.all".to_owned(),
                message: "HGETALL returned an odd number of elements".to_owned(),
            });
        }

        flat.chunks_exact(2)
            .map(|pair| {
                let member  = ProfileId::try_from(pair[0].as_str())?;
                let message = MessageId::try_from(pair[1].as_str())?;
                Ok((member, message))
            })
            .collect()
    }
}
