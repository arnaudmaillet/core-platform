use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::{
    application::port::{EventPublisher, PostRepository},
    domain::{
        aggregate::Post,
        entity::MediaAttachment,
        value_object::{Caption, CdnUrl, MimeType, PostId, PostKind, ProfileId},
    },
    error::PostError,
};

pub struct AttachmentInput {
    pub cdn_url:          String,
    pub mime_type:        String,
    pub width:            u32,
    pub height:           u32,
    pub thumbnail_url:    Option<String>,
    pub duration_seconds: Option<f32>,
}

pub struct CreatePostCommand {
    pub post_id:     String,
    pub profile_id:  String,
    pub kind:        i32,
    pub caption:     String,
    pub attachments: Vec<AttachmentInput>,
    pub parent_id:   Option<String>,
    pub root_id:     Option<String>,
}

impl Command for CreatePostCommand {}

impl Validate for CreatePostCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "PST-VAL-001", "post_id must not be empty"));
        }
        if self.profile_id.trim().is_empty() {
            v.push(FieldViolation::new("profile_id", "PST-VAL-002", "profile_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub(crate) fn parse_attachments(inputs: &[AttachmentInput]) -> Result<Vec<MediaAttachment>, PostError> {
    inputs
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let cdn_url   = CdnUrl::new(&a.cdn_url)?;
            let mime_type = MimeType::new(&a.mime_type)?;
            if a.width == 0 || a.height == 0 {
                return Err(PostError::InvalidDimensions { index: i, width: a.width, height: a.height });
            }
            let thumbnail_url = a.thumbnail_url.as_deref()
                .filter(|s| !s.is_empty())
                .map(CdnUrl::new)
                .transpose()?;
            Ok(MediaAttachment {
                cdn_url,
                mime_type,
                width:            a.width,
                height:           a.height,
                thumbnail_url,
                duration_seconds: a.duration_seconds,
            })
        })
        .collect()
}

pub struct CreatePostHandler<R, P> {
    pub repository: Arc<R>,
    pub publisher:  Arc<P>,
}

impl<R, P> CommandHandler<CreatePostCommand> for CreatePostHandler<R, P>
where
    R: PostRepository,
    P: EventPublisher,
{
    type Error = PostError;

    async fn handle(&self, envelope: Envelope<CreatePostCommand>) -> Result<(), PostError> {
        let cmd = &envelope.payload;

        let post_id    = PostId::try_from(cmd.post_id.as_str())?;
        let profile_id = ProfileId::try_from(cmd.profile_id.as_str())?;

        let kind = match cmd.kind {
            1 => PostKind::TextOnly,
            2 => PostKind::Carousel,
            3 => PostKind::MainVideo,
            v => return Err(PostError::DomainViolation {
                field:   "kind".into(),
                message: format!("unknown proto PostKind value: {v}"),
            }),
        };

        let caption     = Caption::new(&cmd.caption)?;
        let attachments = parse_attachments(&cmd.attachments)?;

        let parent_id = cmd.parent_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(PostId::try_from)
            .transpose()?;
        let root_id = cmd.root_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(PostId::try_from)
            .transpose()?;

        let post = Post::create(post_id, profile_id, kind, caption, attachments, parent_id, root_id)?;
        self.repository.insert(&post).await?;
        Ok(())
    }
}
