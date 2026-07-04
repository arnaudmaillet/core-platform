use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::ScoreStore;
use crate::domain::value_object::PostId;
use crate::error::EngagementError;

pub struct RecordViewCommand {
    pub post_id: String,
}

impl Command for RecordViewCommand {}

impl Validate for RecordViewCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "ENG-VAL-001", "post_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct RecordViewHandler<S> {
    pub score_store: Arc<S>,
}

impl<S: ScoreStore> CommandHandler<RecordViewCommand> for RecordViewHandler<S> {
    type Error = EngagementError;

    async fn handle(&self, envelope: Envelope<RecordViewCommand>) -> Result<(), EngagementError> {
        let post_id = PostId::try_from(envelope.payload.post_id.as_str())?;

        // Single Redis INCR. No ScyllaDB touch; write-behind flush handles durability.
        self.score_store.incr_view(&post_id).await
    }
}
