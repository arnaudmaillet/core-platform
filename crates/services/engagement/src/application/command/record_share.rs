use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::ScoreStore;
use crate::domain::value_object::PostId;
use crate::error::EngagementError;

pub struct RecordShareCommand {
    pub post_id: String,
}

impl Command for RecordShareCommand {}

impl Validate for RecordShareCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "ENG-VAL-001", "post_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct RecordShareHandler<S> {
    pub score_store: Arc<S>,
}

impl<S: ScoreStore> CommandHandler<RecordShareCommand> for RecordShareHandler<S> {
    type Error = EngagementError;

    async fn handle(&self, envelope: Envelope<RecordShareCommand>) -> Result<(), EngagementError> {
        let post_id = PostId::try_from(envelope.payload.post_id.as_str())?;

        self.score_store.incr_share(&post_id).await
    }
}
