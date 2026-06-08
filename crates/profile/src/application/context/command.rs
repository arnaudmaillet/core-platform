use crate::{context::ProfileAppContext, entities::Profile, types::Handle};
use infra_sqlx::TransactionManagerExt;
use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result, TransactionManager, Versioned},
    messaging::{Event, EventEmitter},
    types::{ProfileId, Region},
};
use uuid::Uuid;
#[derive(Clone)]
pub struct ProfileCommandContext<TM> {
    app: ProfileAppContext<TM>,
    profile_id: Option<ProfileId>,
    region: Region,
}

impl<TM> ProfileCommandContext<TM> {
    pub(crate) fn new(
        app: ProfileAppContext<TM>,
        profile_id: Option<ProfileId>,
        region: Region,
    ) -> Self {
        Self {
            app,
            profile_id,
            region,
        }
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub fn profile_id(&self) -> Result<&ProfileId> {
        self.profile_id.as_ref().ok_or_else(|| {
            Error::validation(
                "profile_id",
                "Profile ID is missing in this context (Creation flow)",
            )
        })
    }

    pub async fn exists_by_handle(&self, handle: &Handle) -> Result<bool> {
        self.app
            .profile_repo()
            .exists_by_handle(handle, self.region)
            .await
    }
}

impl<TM: TransactionManager> ProfileCommandContext<TM> {
    pub async fn ensure_creatable(
        &self,
        command_id: Uuid,
        command_region: Region,
        handle: &Handle,
    ) -> Result<bool> {
        if command_region != self.region {
            return Err(Error::validation(
                "region",
                "Region mismatch for profile creation",
            ));
        }

        if !self.ensure_executable(command_id, command_region).await? {
            return Ok(false);
        }

        if self
            .app
            .profile_repo()
            .exists_by_handle(handle, self.region)
            .await?
        {
            return Err(Error::already_exists(
                "Profile",
                "handle",
                handle.as_str().to_string(),
            ));
        }

        Ok(true)
    }

    pub async fn ensure_executable(
        &self,
        command_id: Uuid,
        command_region: Region,
    ) -> Result<bool> {
        if command_region != self.region {
            return Err(Error::validation(
                "region",
                "Region mismatch (sharding violation prevention)",
            ));
        }

        // Affectation directe et typée du retour de la transaction
        let exists = self
            .app
            .transaction_manager()
            .run_in_transaction(|mut tx| async move {
                let is_present = self
                    .app
                    .idempotency_repo()
                    .exists(Some(&mut *tx), &command_id)
                    .await?;
                Ok(is_present)
            })
            .await?;

        Ok(!exists)
    }

    pub async fn fetch_verified(
        &self,
        target: &CommandTarget<ProfileId>,
    ) -> Result<Profile> {
        if Some(&target.id) != self.profile_id.as_ref() {
            return Err(Error::validation("target", "Context/Target mismatch"));
        }

        let profile = self
            .app
            .profile_repo()
            .find_by_id(target.id, self.region, None)
            .await?
            .ok_or_else(|| Error::not_found("Profile", target.id.to_string()))?;

        let expected_version = target.expected_version.ok_or_else(|| {
            Error::validation(
                "expected_version",
                "Sharding strict: Expected version is missing for this transaction",
            )
        })?;

        if profile.version() != expected_version {
            return Err(Error::concurrency_conflict(format!(
                "OCC Mismatch: DB v{}, Expected v{}",
                profile.version(),
                expected_version
            )));
        }

        Ok(profile)
    }

    pub async fn save(&self, profile: &mut Profile, command_id: Option<Uuid>) -> Result<()> {
        if let Some(expected_id) = self.profile_id {
            if profile.profile_id() != expected_id {
                return Err(Error::validation(
                    "profile_id",
                    "Identity mismatch violation",
                ));
            }
        }

        let events = profile.pull_events();

        self.app
            .transaction_manager()
            .run_in_transaction(|mut tx| async move {
                if let Some(cmd_id) = command_id {
                    if self
                        .app
                        .idempotency_repo()
                        .exists(Some(&mut *tx), &cmd_id)
                        .await?
                    {
                        return Err(Error::already_exists("Command", "id", cmd_id.to_string()));
                    }
                }

                self.app
                    .profile_repo()
                    .save(self.region, profile, Some(&mut *tx))
                    .await?;

                if !events.is_empty() {
                    let event_refs: Vec<&dyn Event> = events.iter().map(|e| e.as_ref()).collect();
                    self.app
                        .outbox_repo()
                        .save_all(self.region, &mut *tx, &event_refs)
                        .await?;
                }

                if let Some(cmd_id) = command_id {
                    self.app
                        .idempotency_repo()
                        .save(Some(&mut *tx), &cmd_id)
                        .await?;
                }

                tx.commit().await?;
                Ok(())
            })
            .await?;

        Ok(())
    }
}
