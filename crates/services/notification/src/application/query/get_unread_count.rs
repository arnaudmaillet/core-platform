use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::UnreadCounter;
use crate::domain::value_object::ProfileId;
use crate::error::NotificationError;

pub struct GetUnreadCountQuery {
    pub profile_id: String,
}

impl Query for GetUnreadCountQuery {
    type Response = i64;
}

pub struct GetUnreadCountHandler<U> {
    pub counter: Arc<U>,
}

impl<U: UnreadCounter> QueryHandler<GetUnreadCountQuery> for GetUnreadCountHandler<U> {
    type Error = NotificationError;

    async fn handle(
        &self,
        envelope: Envelope<GetUnreadCountQuery>,
    ) -> Result<i64, NotificationError> {
        let profile_id = ProfileId::try_from(envelope.payload.profile_id.as_str())?;
        self.counter.get(&profile_id).await
    }
}
