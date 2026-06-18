// crates/post/src/domain/types/dimensions.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};

pub const MIN_RESOLUTION: u32 = 144; // Résolution minimale (ex: 144p)
pub const MAX_RESOLUTION: u32 = 3840; // Limite haute en 4K (Ultra HD)

// =========================================================================
// VALUE OBJECT: Width
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct Width(u32);

impl Width {
    pub fn try_new(value: u32) -> Result<Self> {
        let width = Self(value);
        width.validate()?;
        Ok(width)
    }

    pub fn from_raw(value: u32) -> Self {
        Self(value)
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}

impl ValueObject for Width {
    fn validate(&self) -> Result<()> {
        if self.0 < MIN_RESOLUTION || self.0 > MAX_RESOLUTION {
            return Err(Error::validation(
                "width",
                format!(
                    "Width must be between {} and {} pixels",
                    MIN_RESOLUTION, MAX_RESOLUTION
                ),
            ));
        }
        Ok(())
    }
}

// =========================================================================
// VALUE OBJECT: Height
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct Height(u32);

impl Height {
    pub fn try_new(value: u32) -> Result<Self> {
        let height = Self(value);
        height.validate()?;
        Ok(height)
    }

    pub fn from_raw(value: u32) -> Self {
        Self(value)
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}

impl ValueObject for Height {
    fn validate(&self) -> Result<()> {
        if self.0 < MIN_RESOLUTION || self.0 > MAX_RESOLUTION {
            return Err(Error::validation(
                "height",
                format!(
                    "Height must be between {} and {} pixels",
                    MIN_RESOLUTION, MAX_RESOLUTION
                ),
            ));
        }
        Ok(())
    }
}

// =========================================================================
// DOMAIN HELPER: Video Aspect Ratio
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AspectRatio {
    Vertical,   // Format TikTok classique (ex: 9:16)
    Square,     // Format Carré (1:1)
    Horizontal, // Format Paysage (ex: 16:9)
}

impl AspectRatio {
    /// Analyse le ratio entre la largeur et la hauteur
    pub fn from_dimensions(width: Width, height: Height) -> Self {
        let w = width.value() as f32;
        let h = height.value() as f32;
        let ratio = w / h;

        if ratio < 0.95 {
            AspectRatio::Vertical
        } else if ratio > 1.05 {
            AspectRatio::Horizontal
        } else {
            AspectRatio::Square
        }
    }
}

// =========================================================================
// CONVERSIONS (Width)
// =========================================================================

impl TryFrom<u32> for Width {
    type Error = Error;
    fn try_from(value: u32) -> Result<Self> {
        Self::try_new(value)
    }
}

impl TryFrom<i32> for Width {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        if value < 0 {
            return Err(Error::validation("width", "Width cannot be negative"));
        }
        Self::try_new(value as u32)
    }
}

impl From<Width> for u32 {
    fn from(w: Width) -> Self {
        w.0
    }
}

// =========================================================================
// CONVERSIONS (Height)
// =========================================================================

impl TryFrom<u32> for Height {
    type Error = Error;
    fn try_from(value: u32) -> Result<Self> {
        Self::try_new(value)
    }
}

impl TryFrom<i32> for Height {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        if value < 0 {
            return Err(Error::validation("height", "Height cannot be negative"));
        }
        Self::try_new(value as u32)
    }
}

impl From<Height> for u32 {
    fn from(h: Height) -> Self {
        h.0
    }
}
