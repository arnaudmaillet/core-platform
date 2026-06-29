//! Scenario — the Radar vs Focus split.
//!
//! The pan path (`QueryTile`) returns lightweight pins: identity, coordinates,
//! thumbnail — and nothing else. The focus path (`GetGeoTimeline`) hydrates those
//! same posts into full cards carrying the caption and author metadata. This is
//! the read-path axis: the same indexed post is projected two ways depending on
//! interaction stage.

use crate::geo_it::harness::{self, TestHarness, DEADLINE, ZOOM_R9};

// Berlin — indexing location and covering viewport.
const LAT: f64 = 52.5200;
const LNG: f64 = 13.4050;
const SW_LAT: f64 = 52.515;
const SW_LNG: f64 = 13.400;
const NE_LAT: f64 = 52.525;
const NE_LNG: f64 = 13.410;

#[tokio::test]
async fn radar_returns_lean_pin_then_focus_hydrates_the_card() {
    let h = TestHarness::start().await;

    let post = h
        .index_post_full(LAT, LNG, 250.0, "sunset over the Spree", "https://cdn/thumb.jpg")
        .await;

    // ── Radar: the pin is visible in the covering viewport, carrying only the
    //   marker essentials (coordinates + thumbnail), no caption field at all. ──
    harness::await_until("indexed post appears as a Radar pin", DEADLINE, || {
        let h = &h;
        async move {
            let result = h.query_viewport(SW_LAT, SW_LNG, NE_LAT, NE_LNG, ZOOM_R9).await;
            harness::result_contains(&result, &post)
        }
    })
    .await;

    let radar = h.query_viewport(SW_LAT, SW_LNG, NE_LAT, NE_LNG, ZOOM_R9).await;
    let pin = radar
        .pins
        .iter()
        .find(|p| p.post_id == post)
        .expect("pin present");
    assert_eq!(pin.lat, LAT, "pin carries exact latitude, not the H3 cell centroid");
    assert_eq!(pin.lng, LNG, "pin carries exact longitude");
    assert_eq!(pin.thumbnail_url, "https://cdn/thumb.jpg");

    // ── Focus: hydrating the same id yields the full card, including caption. ──
    let focus = h.get_timeline(&[post]).await;
    let card = focus
        .cards
        .iter()
        .find(|c| c.post_id == post)
        .expect("card hydrated on focus");
    assert_eq!(card.caption, "sunset over the Spree", "Focus path serves the caption");
    assert_eq!(card.thumbnail_url, "https://cdn/thumb.jpg");
}

#[tokio::test]
async fn focus_skips_unknown_ids_without_erroring() {
    let h = TestHarness::start().await;

    let known = h
        .index_post_full(LAT, LNG, 250.0, "a caption", "https://cdn/t.jpg")
        .await;
    let unknown = uuid::Uuid::now_v7();

    // Wait until the card projection is durably readable.
    harness::await_until("known post hydrates on focus", DEADLINE, || {
        let h = &h;
        async move { !h.get_timeline(&[known]).await.cards.is_empty() }
    })
    .await;

    let result = h.get_timeline(&[known, unknown]).await;
    assert_eq!(result.cards.len(), 1, "unknown id is silently absent, not an error");
    assert_eq!(result.cards[0].post_id, known);
}
