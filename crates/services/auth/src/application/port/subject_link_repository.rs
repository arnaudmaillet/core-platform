use async_trait::async_trait;

use crate::domain::aggregate::SubjectLink;
use crate::domain::value_object::IdpSubject;
use crate::error::AuthError;

/// Durable persistence port for the immutable IdP-subject → account link.
///
/// `save` must enforce uniqueness on the `(issuer, subject)` pair, surfacing a
/// concurrent first-login race as [`AuthError::SubjectAlreadyLinked`].
#[async_trait]
pub trait SubjectLinkRepository: Send + Sync + 'static {
    async fn find_by_subject(
        &self,
        subject: &IdpSubject,
    ) -> Result<Option<SubjectLink>, AuthError>;

    async fn save(&self, link: &SubjectLink) -> Result<(), AuthError>;
}
