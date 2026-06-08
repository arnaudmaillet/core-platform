// crates/post/src/application/context/command.rs

use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result},
    messaging::EventEmitter,
    types::{PostId, ProfileId, Region},
};
use uuid::Uuid;

use crate::{context::PostAppContext, entities::Post};

#[derive(Clone)]
pub struct PostCommandContext {
    app: PostAppContext,
    author_id: ProfileId,
    region: Region,
}

impl PostCommandContext {
    pub fn new(app: PostAppContext, author_id: ProfileId, region: Region) -> Self {
        Self {
            app,
            author_id,
            region,
        }
    }

    pub fn app(&self) -> &PostAppContext {
        &self.app
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub async fn save_idempotency(&self, command_id: Uuid) -> Result<()> {
        self.app.idempotency_repo().save(None, &command_id).await?;
        Ok(())
    }

    pub async fn ensure_executable(
        &self,
        command_id: Uuid,
        command_region: Region,
    ) -> Result<bool> {
        if command_region != self.region {
            return Err(Error::validation(
                "region",
                &format!(
                    "Sharding violation: Mismatch '{}' vs '{}'",
                    command_region, self.region
                ),
            ));
        }
        let exists = self
            .app
            .idempotency_repo()
            .exists(None, &command_id)
            .await?;
        Ok(!exists)
    }

    pub async fn fetch_verified(&self, target: &CommandTarget<PostId>) -> Result<Post> {
        let post = self
            .app
            .post_repo()
            .find_by_id(self.region, &target.id)
            .await?
            .ok_or_else(|| Error::not_found("Post", target.id.to_string()))?;

        Ok(post)
    }

    pub async fn save(&self, post: &mut Post, command_id: Option<Uuid>) -> Result<()> {
        if post.author_id() != self.author_id {
            return Err(Error::forbidden(&format!(
                "Action non autorisée pour {}",
                self.author_id
            )));
        }

        let _events = post.pull_events();
        self.app.post_repo().save(self.region, post).await?;

        if let Some(cmd_id) = command_id {
            self.app.idempotency_repo().save(None, &cmd_id).await?;
        }
        Ok(())
    }

    pub async fn delete(&self, post: &Post, command_id: Uuid) -> Result<()> {
        if post.author_id() != self.author_id {
            return Err(Error::forbidden(&format!(
                "Action non autorisée pour {}",
                self.author_id
            )));
        }

        if self
            .app
            .idempotency_repo()
            .exists(None, &command_id)
            .await?
        {
            return Err(Error::already_exists(
                "Command",
                "id",
                command_id.to_string(),
            ));
        }

        self.app
            .post_repo()
            .delete(self.region, &post.post_id(), &post.author_id())
            .await?;

        // 4. Marquage de l'idempotence
        self.app.idempotency_repo().save(None, &command_id).await?;

        Ok(())
    }
}
