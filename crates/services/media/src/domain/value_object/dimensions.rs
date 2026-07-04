use serde::{Deserialize, Serialize};

use crate::error::MediaError;

/// Pixel dimensions of an image asset or rendition.
///
/// Construction is the **decode-bomb guard**: it rejects zero extents and any
/// image whose pixel count exceeds [`MAX_PIXELS`](Dimensions::MAX_PIXELS), so a
/// maliciously tiny file that decodes to a multi-gigapixel canvas can never be
/// represented (and thus never enters the pipeline). The limit lives in the domain
/// because it is a safety invariant, not a tunable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dimensions {
    width: u32,
    height: u32,
}

impl Dimensions {
    /// ~100 megapixels — comfortably above any legitimate photo, far below a
    /// decompression-bomb canvas.
    pub const MAX_PIXELS: u64 = 100_000_000;

    pub fn new(width: u32, height: u32) -> Result<Self, MediaError> {
        if width == 0 || height == 0 {
            return Err(MediaError::CorruptMedia {
                reason: "image has a zero dimension".into(),
            });
        }
        if u64::from(width) * u64::from(height) > Self::MAX_PIXELS {
            return Err(MediaError::DimensionLimitExceeded);
        }
        Ok(Self { width, height })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_normal_photo() {
        let d = Dimensions::new(4032, 3024).unwrap();
        assert_eq!(d.pixels(), 4032 * 3024);
    }

    #[test]
    fn rejects_zero_extent_as_corrupt() {
        assert!(matches!(
            Dimensions::new(0, 100).unwrap_err(),
            MediaError::CorruptMedia { .. }
        ));
        assert!(matches!(
            Dimensions::new(100, 0).unwrap_err(),
            MediaError::CorruptMedia { .. }
        ));
    }

    #[test]
    fn rejects_a_decode_bomb() {
        assert!(matches!(
            Dimensions::new(60_000, 60_000).unwrap_err(),
            MediaError::DimensionLimitExceeded
        ));
    }
}
