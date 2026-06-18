use crate::{Post, post::context::PostKernelCtx};
use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result, Versioned},
    types::{PostId, ProfileId, Region},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct PostCommandCtx {
    kernel: PostKernelCtx,
    author_id: ProfileId,
    region_cmd: Region,
}

impl PostCommandCtx {
    pub fn new(kernel: PostKernelCtx, author_id: ProfileId, region_cmd: Region) -> Self {
        Self {
            kernel,
            author_id,
            region_cmd,
        }
    }

    pub fn kernel(&self) -> &PostKernelCtx {
        &self.kernel
    }

    pub fn region(&self) -> Region {
        self.region_cmd
    }

    pub fn author_id(&self) -> &ProfileId {
        &self.author_id
    }

    pub fn verify_actors(&self, post_author_id: ProfileId) -> Result<()> {
        if post_author_id != self.author_id {
            return Err(Error::forbidden(&format!(
                "Action non autorisée : l'acteur {} n'est pas l'auteur du post {}",
                self.author_id, post_author_id
            )));
        }
        Ok(())
    }

    pub async fn fetch_verified(&self, target: &CommandTarget<PostId>) -> Result<Post> {
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

        let post = self
            .kernel
            .read_repo()
            .find_by_id(&target.id)
            .await?
            .ok_or_else(|| Error::not_found("Post", target.id.to_string()))?;

        self.verify_actors(post.author_id())?;

        let expected_version = target.expected_version.ok_or_else(|| {
            Error::validation(
                "expected_version",
                "Sharding strict: Expected version is missing for this transaction",
            )
        })?;

        if post.version() != expected_version {
            return Err(Error::concurrency_conflict(format!(
                "OCC Mismatch: DB v{}, Expected v{}",
                post.version(),
                expected_version
            )));
        }

        Ok(post)
    }

    pub async fn save(&self, post: &mut Post, _command_id: Uuid) -> Result<()> {
        self.verify_actors(post.author_id())?;
        self.kernel.write_repo().save(post).await?;

        Ok(())
    }

    pub async fn delete(&self, post: &Post, _command_id: Uuid) -> Result<()> {
        self.verify_actors(post.author_id())?;
        self.kernel
            .write_repo()
            .delete(&post.post_id(), &post.author_id())
            .await?;

        Ok(())
    }
}
