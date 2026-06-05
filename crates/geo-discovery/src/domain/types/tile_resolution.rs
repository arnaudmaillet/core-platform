use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "i32", into = "i32")]
pub struct TileResolution(u8);

impl TileResolution {
    pub const MIN_RESOLUTION: u8 = 0;
    pub const MAX_RESOLUTION: u8 = 15;

    pub fn try_new(value: i32) -> Result<Self> {
        if value < Self::MIN_RESOLUTION as i32 || value > Self::MAX_RESOLUTION as i32 {
            return Err(Error::validation(
                "tile_resolution",
                format!(
                    "Resolution must be between {} and {}",
                    Self::MIN_RESOLUTION,
                    Self::MAX_RESOLUTION
                ),
            ));
        }

        let res = Self(value as u8);
        res.validate()?;
        Ok(res)
    }

    pub fn value(&self) -> i32 {
        self.0 as i32
    }

    pub fn from_client_zoom(zoom: f32) -> Self {
        let res = match zoom {
            z if z < 4.0 => 3,  // Vue macro : Pays / Continents
            z if z < 7.0 => 5,  // Vue régionale
            z if z < 11.0 => 7, // Vue urbaine (Grande ville / Métropole) -> Notre zone chaude
            z if z < 14.0 => 9, // Vue quartier
            _ => 10,                 // Vue rue / hyper-locale (Plafond de sécurité pour l'infra)
        };
        Self(res)
    }

    pub fn from_client_zoom_int(zoom: i32) -> Self {
        Self::from_client_zoom(zoom as f32)
    }
}

impl ValueObject for TileResolution {
    fn validate(&self) -> Result<()> {
        if self.0 < Self::MIN_RESOLUTION || self.0 > Self::MAX_RESOLUTION {
            return Err(Error::validation(
                "tile_resolution",
                format!(
                    "Resolution must be between {} and {}",
                    Self::MIN_RESOLUTION,
                    Self::MAX_RESOLUTION
                ),
            ));
        }
        Ok(())
    }
}
// --- CONVERSIONS ---

impl TryFrom<i32> for TileResolution {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<TileResolution> for i32 {
    fn from(res: TileResolution) -> Self {
        res.value()
    }
}
