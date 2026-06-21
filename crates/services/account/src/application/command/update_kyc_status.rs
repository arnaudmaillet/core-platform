use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};
use uuid::Uuid;

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::domain::value_object::{AccountId, KycStatus};
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct UpdateKycStatusCommand {
    pub account_id: String,
    /// Serialised `KycStatus` variant name (e.g. `"Approved"`, `"Rejected"`).
    pub new_status: String,
    /// UUID of the internal admin account that performed the review.
    pub reviewer_id: String,
}

impl Command for UpdateKycStatusCommand {}

impl Validate for UpdateKycStatusCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.new_status.trim().is_empty() {
            v.push(FieldViolation::new("new_status", "VAL-2030", "new_status must not be empty"));
        }
        if self.reviewer_id.trim().is_empty() {
            v.push(FieldViolation::new(
                "reviewer_id",
                "VAL-2031",
                "reviewer_id must not be empty",
            ));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct UpdateKycStatusHandler {
    repo: Arc<dyn AccountRepository>,
}

impl UpdateKycStatusHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<UpdateKycStatusCommand> for UpdateKycStatusHandler {
    type Error = AccountError;

    async fn handle(
        &self,
        envelope: Envelope<UpdateKycStatusCommand>,
    ) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;

        let new_status = KycStatus::try_from(cmd.new_status.trim())
            .map_err(|_| AccountError::InvalidKycStatus(cmd.new_status.clone()))?;

        let reviewer_uuid = cmd.reviewer_id.trim().parse::<Uuid>().map_err(|_| {
            AccountError::DomainViolation {
                field: "reviewer_id".into(),
                message: "reviewer_id must be a valid UUID".into(),
            }
        })?;
        let reviewer_id = AccountId::from_uuid(reviewer_uuid);

        let mut account = load_account(&self.repo, &cmd.account_id).await?;
        account.update_kyc_status(new_status, reviewer_id, envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
