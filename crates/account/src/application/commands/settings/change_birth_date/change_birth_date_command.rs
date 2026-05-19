// crates/account/src/application/change_birth_date/command.rs

use crate::domain::types::BirthDate;
use chrono::DateTime;
use serde::Deserialize;
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, Region},
};
use shared_proto::account::v1::ChangeBirthDateRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeBirthDateCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub new_birth_date: BirthDate,
}

impl IdentifiableCommand for ChangeBirthDateCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> String {
        self.target.id.to_string()
    }

    fn region(&self) -> String {
        self.target.region.to_string()
    }
}

impl ChangeBirthDateCommand {
    pub fn try_from_proto(req: ChangeBirthDateRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: Region::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };

        let new_birth_date = req
            .new_birth_date
            .ok_or_else(|| Error::validation("new_birth_date", "Missing timestamp"))
            .and_then(|ts| {
                DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
                    .ok_or_else(|| Error::validation("new_birth_date", "Invalid timestamp range"))
            })
            .map(|dt| dt.date_naive())
            .and_then(|date| {
                BirthDate::try_new(date)
                    .map_err(|e| Error::validation("new_birth_date", e.to_string()))
            })?;

        Ok(Self {
            command_id,
            target,
            new_birth_date,
        })
    }
}
