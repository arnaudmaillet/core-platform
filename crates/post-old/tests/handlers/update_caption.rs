// crates/post/src/application/commands/update_caption/tests.rs

#[cfg(test)]
mod tests {
    use post::commands::UpdateCaptionCommand;
    use post::context::PostCommandCtx;
    use post::types::Caption;
    use post_test_utils::PostTestFixture;
    use post_test_utils::assertions::PostRepositoryAsserts;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{ErrorCode, ManagedEntity, Result, Versioned};
    use shared_kernel::types::ProfileId;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_update_caption_handler_success() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Original caption").build().unwrap();
        let version_snapshot = post.version();

        f.given_post(&post).await;

        let command_id = Uuid::new_v4();
        let target_profile = ProfileId::generate();
        let slug = "arnaud".to_string();

        // Configurer le stub pour résoudre "@arnaud" vers target_profile
        let mut map = BTreeMap::new();
        map.insert(slug.clone(), target_profile);
        f.profile_resolver().set_stub_map(map);

        let new_caption =
            Caption::try_from(format!("New caption with @{}", slug).as_str()).unwrap();

        // Utilisation du versionnement strict (OCC) exigé par le PostCommandCtx
        let cmd = UpdateCaptionCommand {
            command_id,
            target: CommandTarget::versioned(f.post_id(), version_snapshot),
            region: f.server_region(),
            new_caption: Some(new_caption),
        };

        // Act
        f.bus()
            .execute::<PostCommandCtx, UpdateCaptionCommand, ()>(f.command_ctx().clone(), cmd)
            .await?;

        // Assert : Vérification via la passerelle d'assertion
        f.post_assertions()
            .assert_post_state(f.post_id(), |p| {
                assert_eq!(
                    p.caption().as_ref().unwrap().to_string(),
                    "New caption with @arnaud"
                );
                assert!(p.mentions().contains(&target_profile));
                assert_eq!(p.version(), version_snapshot + 1); // Incrémentation OCC valide
            })
            .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_update_caption_handler_no_change_skips() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let caption = Caption::try_from("Same caption").unwrap();
        let post = f.builder("Same caption").build().unwrap();
        let version_snapshot = post.version();

        f.given_post(&post).await;
        let command_id = Uuid::new_v4();

        let cmd = UpdateCaptionCommand {
            command_id,
            target: CommandTarget::versioned(f.post_id(), version_snapshot),
            region: f.server_region(),
            new_caption: Some(caption),
        };

        // Act
        f.bus()
            .execute::<PostCommandCtx, UpdateCaptionCommand, ()>(f.command_ctx().clone(), cmd)
            .await?;

        // Assert : Idempotence métier, aucun changement d'état ni d'incrémentation de version
        f.post_assertions()
            .assert_post_state(f.post_id(), |p| {
                assert_eq!(p.version(), version_snapshot);
                assert_eq!(p.lifecycle().updated_at(), post.lifecycle().updated_at());
            })
            .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_update_caption_concurrency_conflict() -> Result<()> {
        // Arrange
        let f = PostTestFixture::new();
        let post = f.builder("Original caption").build().unwrap();
        f.given_post(&post).await;

        let caption = Caption::try_from("Conflict caption").unwrap();

        // Tentative de mise à jour avec une version obsolète
        let cmd = UpdateCaptionCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::versioned(f.post_id(), 99), // Conflit provoqué
            region: f.server_region(),
            new_caption: Some(caption),
        };

        // Act
        let result = f
            .bus()
            .execute::<PostCommandCtx, UpdateCaptionCommand, ()>(f.command_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(e) if e.code == ErrorCode::ConcurrencyConflict
        ));

        // L'ancienne caption reste la vérité de stockage brute
        f.post_assertions()
            .assert_post_state(f.post_id(), |p| {
                assert_eq!(
                    p.caption().as_ref().unwrap().to_string(),
                    "Original caption"
                );
                assert_eq!(p.version(), post.version());
            })
            .await;

        Ok(())
    }
}
