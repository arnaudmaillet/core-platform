//! Server-side validation over real backends: the real probe rejects non-image
//! bytes, and an oversize declaration is rejected at ticket time.

use media::error::MediaError;

use crate::media_it::harness::Harness;

#[tokio::test]
async fn non_image_bytes_are_rejected_by_the_real_probe_on_commit() {
    let h = Harness::start().await;

    // Issue a ticket, then upload garbage (not a decodable image).
    let garbage = b"this is definitely not an image".to_vec();
    let out = h.issue_ticket(garbage.len() as u64, None, false).await.unwrap();
    assert!(h.put_to_url(&out.upload.unwrap().presigned.url, garbage).await);

    // The probe downloads the bytes, fails to recognize a format, and rejects.
    let err = h.commit(out.asset_id).await.unwrap_err();
    assert!(
        matches!(err, MediaError::CorruptMedia { .. } | MediaError::ContentTypeMismatch { .. }),
        "expected a content-probe rejection, got {err:?}"
    );
}

#[tokio::test]
async fn an_oversize_declaration_is_rejected_at_ticket_time() {
    let h = Harness::start().await;
    // PostImage ceiling is 25 MiB; declare more.
    let err = h.issue_ticket(64 * 1024 * 1024, None, false).await.unwrap_err();
    assert!(matches!(err, MediaError::UploadSizeExceeded { .. }));
}
