use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{AudioFeedRepository, AudioFeedStore};
use crate::domain::value_object::{AudioId, AuthorId, PostId};
use crate::error::TimelineError;

pub struct IngestAudioIndexCommand {
    pub audio_id:        String,
    pub post_id:         String,
    pub author_id:       String,
    pub published_at_ms: i64,
}

impl Command for IngestAudioIndexCommand {}

impl Validate for IngestAudioIndexCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.audio_id.trim().is_empty() {
            v.push(FieldViolation::new("audio_id", "TML-VAL-020", "audio_id must not be empty"));
        }
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "TML-VAL-021", "post_id must not be empty"));
        }
        if self.author_id.trim().is_empty() {
            v.push(FieldViolation::new("author_id", "TML-VAL-022", "author_id must not be empty"));
        }
        if self.published_at_ms <= 0 {
            v.push(FieldViolation::new(
                "published_at_ms",
                "TML-VAL-023",
                "published_at_ms must be a positive Unix epoch millisecond timestamp",
            ));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct IngestAudioIndexHandler<AFR, AFS> {
    pub audio_feed_repo:  Arc<AFR>,
    pub audio_feed_store: Arc<AFS>,
    pub audio_feed_cap:   u16,
}

impl<AFR, AFS> CommandHandler<IngestAudioIndexCommand>
    for IngestAudioIndexHandler<AFR, AFS>
where
    AFR: AudioFeedRepository,
    AFS: AudioFeedStore,
{
    type Error = TimelineError;

    async fn handle(
        &self,
        envelope: Envelope<IngestAudioIndexCommand>,
    ) -> Result<(), TimelineError> {
        let cmd = &envelope.payload;

        let audio_id  = AudioId::try_from(cmd.audio_id.as_str())?;
        let post_id   = PostId::try_from(cmd.post_id.as_str())?;
        let author_id = AuthorId::try_from(cmd.author_id.as_str())?;

        if let Err(e) = self
            .audio_feed_repo
            .insert(&audio_id, &post_id, &author_id, cmd.published_at_ms)
            .await
        {
            tracing::warn!(
                audio_id = %audio_id,
                post_id  = %post_id,
                error    = %e,
                "audio feed ScyllaDB insert failed"
            );
            return Err(e);
        }

        if let Err(e) = self
            .audio_feed_store
            .push(&audio_id, &post_id, &author_id, cmd.published_at_ms, self.audio_feed_cap)
            .await
        {
            tracing::warn!(
                audio_id = %audio_id,
                post_id  = %post_id,
                error    = %e,
                "audio feed Redis push failed"
            );
            return Err(e);
        }

        Ok(())
    }
}
