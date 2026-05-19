use async_trait::async_trait;
use chrono::Utc;
use scylla::{
    client::session::Session,
    statement::{batch::Batch, prepared::PreparedStatement},
    value::Counter as ScyllaCounter, // Import du type fort de Scylla pour les compteurs
};
use shared_kernel::{
    core::{Error, Identifier, Result},
    types::{Counter as DomainCounter, ProfileId},
};
use std::sync::Arc;

use crate::entities::ProfileCounters;
use crate::repositories::CounterRepository;

pub struct ScyllaCounterRepository {
    session: Arc<Session>,
    increment_following_stmt: PreparedStatement,
    decrement_following_stmt: PreparedStatement,
    increment_followers_stmt: PreparedStatement,
    decrement_followers_stmt: PreparedStatement,
    get_counters_stmt: PreparedStatement,
    save_delta_stmt: PreparedStatement,
}

impl ScyllaCounterRepository {
    pub async fn new(session: Arc<Session>) -> Result<Self> {
        let increment_following_stmt = session.prepare(
            "UPDATE profile_counters SET following_count = following_count + 1 WHERE profile_id = ?"
        ).await.map_err(|e| Error::internal(format!("Prepare increment_following failed: {}", e)))?;

        let decrement_following_stmt = session.prepare(
            "UPDATE profile_counters SET following_count = following_count - 1 WHERE profile_id = ?"
        ).await.map_err(|e| Error::internal(format!("Prepare decrement_following failed: {}", e)))?;

        let increment_followers_stmt = session.prepare(
            "UPDATE profile_counters SET followers_count = followers_count + 1 WHERE profile_id = ?"
        ).await.map_err(|e| Error::internal(format!("Prepare increment_followers failed: {}", e)))?;

        let decrement_followers_stmt = session.prepare(
            "UPDATE profile_counters SET followers_count = followers_count - 1 WHERE profile_id = ?"
        ).await.map_err(|e| Error::internal(format!("Prepare decrement_followers failed: {}", e)))?;

        let get_counters_stmt = session.prepare(
            "SELECT followers_count, following_count FROM profile_counters WHERE profile_id = ? LIMIT 1"
        ).await.map_err(|e| Error::internal(format!("Prepare select failed: {}", e)))?;

        let save_delta_stmt = session.prepare(
            "UPDATE profile_counters SET followers_count = followers_count + ?, following_count = following_count + ? WHERE profile_id = ?"
        ).await.map_err(|e| Error::internal(format!("Prepare save_delta failed: {}", e)))?;

        Ok(Self {
            session,
            increment_following_stmt,
            decrement_following_stmt,
            increment_followers_stmt,
            decrement_followers_stmt,
            get_counters_stmt,
            save_delta_stmt,
        })
    }
}

#[async_trait]
impl CounterRepository for ScyllaCounterRepository {
    async fn increment_counters(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<()> {
        let mut batch = Batch::new(scylla::statement::batch::BatchType::Counter);

        batch.append_statement(self.increment_following_stmt.clone());
        batch.append_statement(self.increment_followers_stmt.clone());

        let values = ((follower_id.as_uuid(),), (following_id.as_uuid(),));

        self.session
            .batch(&batch, values)
            .await
            .map_err(|e| Error::internal(format!("Counters increment batch failed: {}", e)))?;

        Ok(())
    }

    async fn decrement_counters(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<()> {
        let mut batch = Batch::new(scylla::statement::batch::BatchType::Counter);

        batch.append_statement(self.decrement_following_stmt.clone());
        batch.append_statement(self.decrement_followers_stmt.clone());

        let values = ((follower_id.as_uuid(),), (following_id.as_uuid(),));

        self.session
            .batch(&batch, values)
            .await
            .map_err(|e| Error::internal(format!("Counters decrement batch failed: {}", e)))?;

        Ok(())
    }

    async fn get_counters(&self, profile_id: ProfileId) -> Result<ProfileCounters> {
        let res = self
            .session
            .execute_unpaged(&self.get_counters_stmt, (profile_id.as_uuid(),))
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        let rows_result = res
            .into_rows_result()
            .map_err(|e| Error::internal(format!("Invalid rows result for counters: {}", e)))?;

        if let Some((followers_opt, following_opt)) = rows_result
            .maybe_first_row::<(Option<ScyllaCounter>, Option<ScyllaCounter>)>()
            .map_err(|e| Error::internal(format!("Counter decoding error: {}", e)))?
        {
            let followers_raw = followers_opt.map(|c| c.0).unwrap_or(0);
            let following_raw = following_opt.map(|c| c.0).unwrap_or(0);

            let followers_count = followers_raw.try_into()?;
            let following_count = following_raw.try_into()?;

            return Ok(ProfileCounters::restore(
                profile_id,
                followers_count,
                following_count,
                1,
                Utc::now(),
            ));
        }

        Ok(ProfileCounters::restore(
            profile_id,
            DomainCounter::default(),
            DomainCounter::default(),
            1,
            Utc::now(),
        ))
    }

    async fn save(&self, counters: &ProfileCounters) -> Result<()> {
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
