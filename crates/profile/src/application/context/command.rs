// crates/profile/src/context/command_context.rs

use crate::{
    context::ProfileKernelCtx, entities::Profile, repositories::ProfileRoutingRepository,
    types::Handle,
};
use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result, Versioned},
    types::{ProfileId, Region},
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ProfileCommandCtx {
    kernel: ProfileKernelCtx,
    profile_id: Option<ProfileId>,
    region_cmd: Region,
}

impl ProfileCommandCtx {
    pub fn new(
        kernel: ProfileKernelCtx,
        profile_id: Option<ProfileId>,
        region_cmd: Region,
    ) -> Self {
        Self {
            kernel,
            profile_id,
            region_cmd,
        }
    }
    
    pub fn server_region(&self) -> Region {
        self.kernel.server_region()
    }

    pub fn profile_id(&self) -> Option<ProfileId> {
        self.profile_id
    }

    pub fn routing_repo(&self) -> Arc<dyn ProfileRoutingRepository> {
        self.kernel.routing_repo()
    }

    /// Valide l'adéquation entre l'identité de la commande et l'agrégat manipulé
    pub fn verify_actors(&self, target_id: ProfileId) -> Result<()> {
        if let Some(expected_id) = self.profile_id {
            if target_id != expected_id {
                return Err(Error::forbidden(&format!(
                    "Action non autorisée : l'acteur {} tente de modifier le profil {}",
                    expected_id, target_id
                )));
            }
        }

        Ok(())
    }

    /// Empêche l'exécution d'une transaction si la commande cible la mauvaise région de Sharding
    fn verify_region_sharding(&self) -> Result<()> {
        if self.region_cmd != self.kernel.server_region() {
            return Err(Error::validation(
                "region",
                format!(
                    "Sharding violation prevention: Command region '{}' mismatch with deployment cluster region '{}'",
                    self.region_cmd,
                    self.kernel.server_region()
                ),
            ));
        }
        Ok(())
    }

    pub async fn exists_by_handle(&self, handle: &Handle) -> Result<bool> {
        let slug_hash = handle.to_sha256_hash();
        let res = self.routing_repo().resolve_slug(&slug_hash).await?;
        Ok(res.is_some())
    }

    pub async fn fetch_verified(&self, target: &CommandTarget<ProfileId>) -> Result<Profile> {
        self.verify_region_sharding()?;
        self.verify_actors(target.id)?;

        let profile: Profile = self
            .kernel
            .profile_repo()
            .find_by_id(target.id)
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

    pub async fn save(&self, profile: &mut Profile, command_id: Uuid) -> Result<()> {
        // Validation préventive du Shard régional et des acteurs
        self.verify_region_sharding()?;
        self.verify_actors(profile.profile_id())?;

        let idempotency_repo = self.kernel.idempotency_repo();

        // Vérification de l'idempotence au niveau du Shard
        let already_processed = idempotency_repo.exists(None, &command_id).await?;
        if already_processed {
            tracing::warn!(
                command_id = %command_id,
                "Idempotence DB : Commande de profil déjà appliquée sur ce Shard. Skip."
            );
            return Ok(());
        }

        // Persistance effective de l'état de l'agrégat
        self.kernel.profile_repo().save(profile).await?;

        // Enregistrement du token d'idempotence après succès
        idempotency_repo.save(None, &command_id).await?;

        Ok(())
    }
}
