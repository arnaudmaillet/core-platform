use post::{DurationSeconds, Height, MediaBuilder, MediaId, MediaType, MimeType, Width};
use shared_kernel::{core::ValueObject, types::Url};

fn valid_params() -> (
    MediaId,
    Url,
    Url,
    DurationSeconds,
    Width,
    Height,
    MediaType,
    MimeType,
) {
    (
        MediaId::generate(),
        Url::try_new("https://cdn.wynn.com/video.mp4").unwrap(),
        Url::try_new("https://cdn.wynn.com/thumb.jpg").unwrap(),
        DurationSeconds::try_new(60).unwrap(),
        Width::try_new(1920).unwrap(),
        Height::try_new(1080).unwrap(),
        MediaType::Video,
        MimeType::try_from("video/mp4").unwrap(),
    )
}

#[test]
fn test_media_asset_valid_video() {
    let (id, url, thumb, dur, w, h, mtype, mime) = valid_params();
    let asset = MediaBuilder::new(id, url, thumb, dur, w, h, mtype, mime)
        .build()
        .unwrap();

    assert!(asset.validate().is_ok());
}

#[test]
fn test_media_asset_valid_image() {
    let asset = MediaBuilder::new(
        MediaId::generate(),
        Url::try_new("https://cdn.wynn.com/img.jpg").unwrap(),
        Url::try_new("https://cdn.wynn.com/thumb.jpg").unwrap(),
        DurationSeconds::try_new(0).unwrap(),
        Width::try_new(1080).unwrap(),
        Height::try_new(1080).unwrap(),
        MediaType::Image,
        MimeType::try_from("image/jpeg").unwrap(),
    )
    .build()
    .unwrap();

    assert!(asset.validate().is_ok());
}

#[test]
fn test_media_asset_image_with_duration_fails() {
    let result = MediaBuilder::new(
        MediaId::generate(),
        Url::try_new("https://cdn.wynn.com/img.jpg").unwrap(),
        Url::try_new("https://cdn.wynn.com/thumb.jpg").unwrap(),
        DurationSeconds::try_new(1).unwrap(),
        Width::try_new(1080).unwrap(),
        Height::try_new(1080).unwrap(),
        MediaType::Image,
        MimeType::try_from("image/jpeg").unwrap(),
    )
    .build();

    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(
        err.details.unwrap()["reason"]
            .as_str()
            .unwrap()
            .contains("strictly 0 seconds")
    );
}

#[test]
fn test_media_asset_type_mismatch_fails() {
    let result = MediaBuilder::new(
        MediaId::generate(),
        Url::try_new("https://cdn.wynn.com/vid.mp4").unwrap(),
        Url::try_new("https://cdn.wynn.com/thumb.jpg").unwrap(),
        DurationSeconds::try_new(10).unwrap(),
        Width::try_new(1920).unwrap(),
        Height::try_new(1080).unwrap(),
        MediaType::Video,
        MimeType::try_from("image/jpeg").unwrap(),
    )
    .build();

    assert!(result.is_err());
    assert!(
        result.unwrap_err().details.unwrap()["reason"]
            .as_str()
            .unwrap()
            .contains("is not a video format")
    );
}
