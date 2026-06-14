// crates/post/src/application/commands/create_post/tests.rs

#[cfg(test)]
mod tests {
    use post::commands::CreatePostCommand;
    use post::context::PostCommandCtx;
    use post::types::Caption;
    use post_test_utils::PostTestFixture;
    use post_test_utils::assertions::PostRepositoryAsserts;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{Result, Versioned};
    use shared_kernel::idempotency::IdempotencyRepository;
    use shared_kernel::types::{PostType, ProfileId};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_create_post_handler_success_with_mentions() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let command_id = Uuid::new_v4();
        let target_profile = ProfileId::generate();
        let slug = "arnaud".to_string();

        // Configuration du stub de résolution des pseudos
        let mut map = BTreeMap::new();
        map.insert(slug.clone(), target_profile);
        f.profile_resolver().set_stub_map(map);

        let caption = Caption::try_from(format!("Hello @{}", slug).as_str()).unwrap();

        // Pour une création, la cible de la commande identifie l'auteur de manière stateless
        let target = CommandTarget::stateless(f.author_id());

        let cmd = CreatePostCommand {
            command_id,
            target,
            region: f.server_region(),
            post_id: f.post_id(),
            post_type: PostType::Text,
            caption: Some(caption),
            media_list: vec![],
            visibility_level: "Public".to_string(),
            allowed_comment_hands: true,
            dynamic_metadata: None,
            music_id: None,
        };

        // Act
        f.bus()
            .execute::<PostCommandCtx, CreatePostCommand, ()>(f.command_ctx().clone(), cmd)
            .await?;

        // Assert : Utilisation de la passerelle d'assertions pour valider l'état brut créé
        f.post_assertions()
            .assert_post_state(f.post_id(), |post| {
                assert_eq!(post.post_id(), f.post_id());
                assert_eq!(post.author_id(), f.author_id());
                assert_eq!(post.version(), 1);
                assert!(post.mentions().contains(&target_profile));
            })
            .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_create_post_handler_idempotency_barrier() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let command_id = Uuid::new_v4();
        f.idempotency_repo().save(None, &command_id).await?;

        let target = CommandTarget::stateless(f.author_id());

        let cmd = CreatePostCommand {
            command_id,
            target,
            region: f.server_region(),
            post_id: f.post_id(),
            post_type: PostType::Text,
            caption: None,
            media_list: vec![],
            visibility_level: "Public".to_string(),
            allowed_comment_hands: true,
            dynamic_metadata: None,
            music_id: None,
        };

        // Act
        f.bus()
            .execute::<PostCommandCtx, CreatePostCommand, ()>(f.command_ctx().clone(), cmd)
            .await?;

        // Assert : Bloqué par l'idempotence, le post ne doit jamais avoir été instancié ou persisté
        f.post_assertions().assert_not_found(f.post_id()).await;

        Ok(())
    }
}
