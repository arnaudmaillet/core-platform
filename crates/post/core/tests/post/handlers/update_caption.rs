// crates/post/core/tests/post/handlers/toggle_comments.rs

use post::{Caption, PostCommandCtx, UpdateCaptionCommand};
use post_utils::{PostRepositoryAsserts, PostTestFixture};
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, ManagedEntity, Result, Versioned};
use uuid::Uuid;

// #[tokio::test]
// async fn test_update_caption_handler_success() -> Result<()> {
//     // Arrange
//     let f = PostTestFixture::new();
//     let post = f.builder("Original caption").build().unwrap();
//     let version_snapshot = post.version();

//     f.given_post(&post).await;

//     let command_id = Uuid::new_v4();
//     let slug = "arnaud".to_string();

//     // 1. Suppression du stub map de profile_resolver qui n'existe plus.
//     // Si ton handler ou ta commande prend désormais un champ dédié pour les mentions
//     // ou si le comportement a changé, tu adaptes la struct ici.
//     let new_caption = Caption::try_from(format!("New caption with @{}", slug).as_str()).unwrap();

//     // Utilisation du versionnement strict (OCC) exigé par le PostCommandCtx
//     let cmd = UpdateCaptionCommand {
//         command_id,
//         target: CommandTarget::versioned(f.post_id(), version_snapshot),
//         region: f.server_region(),
//         new_caption: Some(new_caption),
//     };

//     // Act
//     f.bus()
//         .execute::<PostCommandCtx, UpdateCaptionCommand, ()>(f.command_ctx().clone(), cmd)
//         .await?;

//     // Assert : Vérification via la passerelle d'assertion
//     f.post_assertions()
//         .assert_post_state(f.post_id(), |p| {
//             assert_eq!(
//                 p.caption().as_ref().unwrap().to_string(),
//                 "New caption with @arnaud"
//             );
//             // Si ton entité extrait toujours la mention de manière autonome par parsing texte :
//             // assert!(p.mentions().contains(&target_profile));
//             assert_eq!(p.version(), version_snapshot + 1); // Incrémentation OCC valide
//         })
//         .await;

//     Ok(())
// }

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

// #[tokio::test]
// async fn test_update_caption_concurrency_conflict() -> Result<()> {
//     // Arrange
//     let f = PostTestFixture::new();
//     let post = f.builder("Original caption").build().unwrap();
//     f.given_post(&post).await;

//     let caption = Caption::try_from("Conflict caption").unwrap();

//     // Tentative de mise à jour avec une version obsolète
//     let cmd = UpdateCaptionCommand {
//         command_id: Uuid::new_v4(),
//         target: CommandTarget::versioned(f.post_id(), 99), // Conflit provoqué
//         region: f.server_region(),
//         new_caption: Some(caption),
//     };

//     // Act
//     let result = f
//         .bus()
//         .execute::<PostCommandCtx, UpdateCaptionCommand, ()>(f.command_ctx().clone(), cmd)
//         .await;

//     // Assert
//     assert!(matches!(
//         result,
//         Err(e) if e.code == ErrorCode::ConcurrencyConflict
//     ));

//     // L'ancienne caption reste la vérité de stockage brute
//     f.post_assertions()
//         .assert_post_state(f.post_id(), |p| {
//             assert_eq!(
//                 p.caption().as_ref().unwrap().to_string(),
//                 "Original caption"
//             );
//             assert_eq!(p.version(), post.version());
//         })
//         .await;

//     Ok(())
// }
