#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use post::entities::MediaAsset;
    use post::entities::Post;
    use post::types::{
        Caption, DurationSeconds, Height, MediaType, MimeType, PostType, VisibilityLevel, Width,
    };
    use post::types::{MediaId, Mentions};
    use shared_kernel::messaging::EventEmitter;
    use shared_kernel::types::{PostId, ProfileId, Region, Url};

    // Helper pour créer un MediaAsset valide
    fn create_video_asset(id: MediaId) -> MediaAsset {
        MediaAsset::builder(
            id,
            Url::try_new("https://cdn.wynn.com/v.mp4").unwrap(),
            Url::try_new("https://cdn.wynn.com/t.jpg").unwrap(),
            DurationSeconds::try_new(60).unwrap(),
            Width::try_new(1920).unwrap(),
            Height::try_new(1080).unwrap(),
            MediaType::Video,
            MimeType::try_from("video/mp4").unwrap(),
        )
        .build()
        .unwrap()
    }

    fn create_image_asset(id: MediaId) -> MediaAsset {
        MediaAsset::builder(
            id,
            Url::try_new("https://cdn.wynn.com/i.jpg").unwrap(),
            Url::try_new("https://cdn.wynn.com/t.jpg").unwrap(),
            DurationSeconds::try_new(0).unwrap(),
            Width::try_new(1080).unwrap(),
            Height::try_new(1080).unwrap(),
            MediaType::Image,
            MimeType::try_from("image/jpeg").unwrap(),
        )
        .build()
        .unwrap()
    }

    #[test]
    fn test_post_creation_invariants_text_no_media() {
        let post = Post::builder(
            PostId::generate(),
            ProfileId::generate(),
            PostType::Text,
            VisibilityLevel::Public,
        )
        .with_caption(Caption::try_from("Hello").unwrap())
        .build()
        .unwrap();
        assert!(post.media_list().is_empty());
    }

    #[test]
    fn test_post_creation_image_without_caption_is_valid() {
        let asset = create_image_asset(MediaId::generate());

        // Test : Un post Image peut avoir une légende vide (None)
        let post = Post::builder(
            PostId::generate(),
            ProfileId::generate(),
            PostType::Image,
            VisibilityLevel::Public,
        )
        .with_media_list(vec![asset])
        .build()
        .unwrap();

        assert!(post.caption().is_none());
    }

    #[test]
    fn test_post_creation_invariants_text_with_media_fails() {
        let asset = create_video_asset(MediaId::generate());

        let result = Post::builder(
            PostId::generate(),
            ProfileId::generate(),
            PostType::Text,
            VisibilityLevel::Public,
        )
        .with_caption(Caption::try_from("Hello").unwrap())
        .with_media_list(vec![asset])
        .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_post_duration_calculation() {
        let asset1 = create_video_asset(MediaId::generate()); // 60s
        let asset2 = create_video_asset(MediaId::generate()); // 60s
        let post = Post::builder(
            PostId::generate(),
            ProfileId::generate(),
            PostType::Carousel,
            VisibilityLevel::Public,
        )
        .with_caption(Caption::try_from("Cool carousel").unwrap())
        .with_media_list(vec![asset1, asset2])
        .build()
        .unwrap();

        assert_eq!(post.total_duration_seconds(), 120);
    }

    #[test]
    fn test_update_caption_extracts_hashtags() {
        let mut post = Post::builder(
            PostId::generate(),
            ProfileId::generate(),
            PostType::Text,
            VisibilityLevel::Public,
        )
        .with_caption(Caption::try_from("Original").unwrap())
        .build()
        .unwrap();

        let new_caption = Caption::try_from("Check this #rust #ddd").unwrap();

        post.update_caption(Some(new_caption), Mentions::empty())
            .unwrap();

        assert_eq!(post.hashtags().len(), 2);
        assert!(post.is_edited());
    }

    #[test]
    fn test_update_caption_extracts_mentions() {
        let author_id = ProfileId::generate();
        let target_profile_1 = ProfileId::generate();
        let target_profile_2 = ProfileId::generate();

        let mut post = Post::builder(
            PostId::generate(),
            author_id,
            PostType::Text,
            VisibilityLevel::Public,
        )
        .with_caption(Caption::try_from("Original").unwrap())
        .build()
        .unwrap();

        let mut resolved_profiles = BTreeSet::new();
        resolved_profiles.insert(target_profile_1);
        resolved_profiles.insert(target_profile_2);

        let new_mentions = Mentions::try_new(resolved_profiles).unwrap();

        let new_caption_text = format!(
            "Hey @{} and @{} check out the new Wynn release!",
            target_profile_1, target_profile_2
        );
        let new_caption = Caption::try_from(new_caption_text.as_str()).unwrap();

        post.update_caption(Some(new_caption), new_mentions)
            .unwrap();

        assert_eq!(post.mentions().len(), 2);
        assert!(post.mentions().contains(&target_profile_1));
        assert!(post.mentions().contains(&target_profile_2));
        assert!(post.is_edited());
    }

    #[test]
    fn test_visibility_and_comments_toggles() {
        let mut post = Post::builder(
            PostId::generate(),
            ProfileId::generate(),
            PostType::Text,
            VisibilityLevel::Public,
        )
        .with_caption(Caption::try_from("Hi").unwrap())
        .build()
        .unwrap();

        post.change_visibility(VisibilityLevel::Private).unwrap();
        assert_eq!(post.visibility_level(), VisibilityLevel::Private);

        post.toggle_comments(false).unwrap();
        assert!(!post.allowed_comment_hands());
    }

    #[test]
    fn test_events_emitted_on_mutation() {
        let mut post = Post::builder(
            PostId::generate(),
            ProfileId::generate(),
            PostType::Text,
            VisibilityLevel::Public,
        )
        .with_caption(Caption::try_from("Hi").unwrap())
        .build()
        .unwrap();

        post.toggle_comments(false).unwrap();

        let events = post.pull_events();
        assert_eq!(events.len(), 1);
    }
}
