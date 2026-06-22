use error::{AppError, Severity};
use http::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TimelineError {
    #[error(transparent)]
    Scylla(#[from] scylla_storage::ScyllaStorageError),

    #[error(transparent)]
    Redis(#[from] redis_storage::RedisStorageError),

    #[error(transparent)]
    Validation(#[from] validation::ValidationError),

    // ── TML-1xxx: Feed state violations ───────────────────────────────────────
    #[error("feed not found for profile {profile_id}")]
    FeedNotFound { profile_id: String },

    // ── TML-2xxx: Fan-out routing errors ──────────────────────────────────────
    #[error("fan-out failed for author {author_id}: {message}")]
    FanOutFailed { author_id: String, message: String },

    #[error("VIP registry write failed for author {author_id}: {message}")]
    VipRegistryWriteFailed { author_id: String, message: String },

    // ── TML-3xxx: Social graph client errors ──────────────────────────────────
    #[error("social-graph gRPC call failed: {message}")]
    SocialGraphClientError { message: String },

    #[error("social-graph returned an invalid profile ID: '{0}'")]
    SocialGraphInvalidId(String),

    // ── TML-4xxx: Cold-start / hydration errors ───────────────────────────────
    #[error("cold-start hydration failed for profile {profile_id}: {message}")]
    ColdStartFailed { profile_id: String, message: String },

    // ── TML-5xxx: Worker / background task errors ─────────────────────────────
    #[error("Lua script returned an unexpected value in context '{context}'")]
    ScriptReturnInvalid { context: &'static str },

    #[error("backfill failed for follower {follower_id} / followee {followee_id}: {message}")]
    BackfillFailed { follower_id: String, followee_id: String, message: String },

    // ── TML-6xxx: Pagination errors ───────────────────────────────────────────
    #[error("invalid page token: '{token}'")]
    InvalidPageToken { token: String },

    // ── TML-9xxx: ID parsing / domain violations ──────────────────────────────
    #[error("invalid post ID: '{0}'")]
    InvalidPostId(String),

    #[error("invalid profile ID: '{0}'")]
    InvalidProfileId(String),

    #[error("invalid author ID: '{0}'")]
    InvalidAuthorId(String),

    #[error("domain violation on field '{field}': {message}")]
    DomainViolation { field: String, message: String },
}

impl AppError for TimelineError {
    fn error_code(&self) -> &'static str {
        match self {
            Self::Scylla(e)     => e.error_code(),
            Self::Redis(e)      => e.error_code(),
            Self::Validation(e) => e.error_code(),

            Self::FeedNotFound { .. }           => "TML-1001",

            Self::FanOutFailed { .. }           => "TML-2001",
            Self::VipRegistryWriteFailed { .. } => "TML-2002",

            Self::SocialGraphClientError { .. } => "TML-3001",
            Self::SocialGraphInvalidId(_)       => "TML-3002",

            Self::ColdStartFailed { .. }        => "TML-4001",

            Self::ScriptReturnInvalid { .. }    => "TML-5001",
            Self::BackfillFailed { .. }         => "TML-5002",

            Self::InvalidPageToken { .. }       => "TML-6001",

            Self::InvalidPostId(_)              => "TML-9001",
            Self::InvalidProfileId(_)           => "TML-9002",
            Self::InvalidAuthorId(_)            => "TML-9003",
            Self::DomainViolation { .. }        => "TML-9004",
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            Self::Scylla(e)     => e.http_status(),
            Self::Redis(e)      => e.http_status(),
            Self::Validation(e) => e.http_status(),

            Self::FeedNotFound { .. } => StatusCode::NOT_FOUND,

            Self::InvalidPageToken { .. }
            | Self::InvalidPostId(_)
            | Self::InvalidProfileId(_)
            | Self::InvalidAuthorId(_)
            | Self::DomainViolation { .. } => StatusCode::UNPROCESSABLE_ENTITY,

            Self::FanOutFailed { .. }
            | Self::VipRegistryWriteFailed { .. }
            | Self::SocialGraphClientError { .. }
            | Self::SocialGraphInvalidId(_)
            | Self::ColdStartFailed { .. }
            | Self::ScriptReturnInvalid { .. }
            | Self::BackfillFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::Scylla(e) => e.severity(),
            Self::Redis(e)  => e.severity(),

            Self::FanOutFailed { .. }
            | Self::VipRegistryWriteFailed { .. }
            | Self::ColdStartFailed { .. }
            | Self::ScriptReturnInvalid { .. }
            | Self::BackfillFailed { .. } => Severity::High,

            Self::SocialGraphClientError { .. } => Severity::High,

            Self::Validation(e) => e.severity(),

            Self::SocialGraphInvalidId(_)
            | Self::DomainViolation { .. } => Severity::Medium,

            Self::FeedNotFound { .. }
            | Self::InvalidPageToken { .. }
            | Self::InvalidPostId(_)
            | Self::InvalidProfileId(_)
            | Self::InvalidAuthorId(_) => Severity::Low,
        }
    }

    fn is_retryable(&self) -> bool {
        match self {
            Self::Scylla(e) => e.is_retryable(),
            Self::Redis(e)  => e.is_retryable(),
            Self::SocialGraphClientError { .. } => true,
            _ => false,
        }
    }

    fn category(&self) -> &'static str {
        match self {
            Self::Scylla(e)     => e.category(),
            Self::Redis(e)      => e.category(),
            Self::Validation(e) => e.category(),
            _                   => "TML",
        }
    }

    fn user_facing_message(&self) -> &'static str {
        match self {
            Self::Scylla(_)
            | Self::Redis(_)
            | Self::FanOutFailed { .. }
            | Self::VipRegistryWriteFailed { .. }
            | Self::SocialGraphClientError { .. }
            | Self::SocialGraphInvalidId(_)
            | Self::ColdStartFailed { .. }
            | Self::ScriptReturnInvalid { .. }
            | Self::BackfillFailed { .. } =>
                "An internal error occurred. Please try again later.",

            Self::FeedNotFound { .. } =>
                "No feed was found for this profile.",

            Self::InvalidPageToken { .. } =>
                "The pagination cursor is invalid. Please restart from the first page.",

            Self::InvalidPostId(_)    => "The provided post ID is not valid.",
            Self::InvalidProfileId(_) => "The provided profile ID is not valid.",
            Self::InvalidAuthorId(_)  => "The provided author ID is not valid.",
            Self::DomainViolation { .. } =>
                "The request contains an invalid domain value.",

            Self::Validation(e) => e.user_facing_message(),
        }
    }
}
