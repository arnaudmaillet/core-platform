use async_trait::async_trait;
use chrono::Utc;
use infra_scylla::scylla::{
    client::session::Session, statement::prepared::PreparedStatement,
    value::Counter as ScyllaCounter,
};
use shared_kernel::{
    core::{Error, Identifier, Result},
    types::ProfileId,
};
use std::sync::Arc;

use crate::entities::ProfileCounters;
use crate::repositories::ProfileCountersStorageRepository;

pub struct ScyllaProfileCountersStore {
    session: Arc<Session>,
    get_counters_stmt: PreparedStatement,
    save_delta_stmt: PreparedStatement,
}

impl ScyllaProfileCountersStore {
    pub async fn new(session: Arc<Session>) -> Result<Self> {
        let get_counters_stmt = session.prepare(
            "SELECT followers_count, following_count FROM profile_counters WHERE profile_id = ? LIMIT 1"
        ).await.map_err(|e| Error::internal(format!("Prepare select failed: {}", e)))?;

        let save_delta_stmt = session.prepare(
            "UPDATE profile_counters SET followers_count = followers_count + ?, following_count = following_count + ? WHERE profile_id = ?"
        ).await.map_err(|e| Error::internal(format!("Prepare save_delta failed: {}", e)))?;

        Ok(Self {
            session,
            get_counters_stmt,
            save_delta_stmt,
        })
    }
}

#[async_trait]
impl ProfileCountersStorageRepository for ScyllaProfileCountersStore {
    async fn fetch(&self, profile_id: ProfileId) -> Result<Option<ProfileCounters>> {
        let res = self
            .session
            .execute_unpaged(&self.get_counters_stmt, (profile_id.as_uuid(),))
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        let rows_result = res
            .into_rows_result()
            .map_err(|e| Error::internal(format!("Invalid rows result for counters: {}", e)))?;

        let now = Utc::now();

        if let Some((followers_opt, following_opt)) = rows_result
            .maybe_first_row::<(Option<ScyllaCounter>, Option<ScyllaCounter>)>()
            .map_err(|e| Error::internal(format!("Counter decoding error: {}", e)))?
        {
            let followers_raw = followers_opt.map(|c| c.0).unwrap_or(0);
            let following_raw = following_opt.map(|c| c.0).unwrap_or(0);

            let followers_count = followers_raw.try_into()?;
            let following_count = following_raw.try_into()?;

            return Ok(Some(ProfileCounters::restore(
                profile_id,
                followers_count,
                following_count,
                now,
            )));
        }
        Ok(None)
    }

    async fn commit_deltas(&self, counters: &ProfileCounters) -> Result<()> {
        let followers_delta = counters.followers_count().value() as i64;
        let following_delta = counters.following_count().value() as i64;

        if followers_delta == 0 && following_delta == 0 {
            return Ok(());
        }

        let followers_cql = ScyllaCounter(followers_delta);
        let following_cql = ScyllaCounter(following_delta);

        self.session
            .execute_unpaged(
                &self.save_delta_stmt,
                (
                    followers_cql,
                    following_cql,
                    counters.profile_id().as_uuid(),
                ),
            )
            .await
            .map_err(|e| Error::internal(format!("ScyllaDB counter update failed: {}", e)))?;

        Ok(())
    }
}
