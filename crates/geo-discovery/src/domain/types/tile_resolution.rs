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

    /// Détermine dynamiquement la résolution Uber H3 appropriée selon le niveau de zoom du client (0.0 - 20.0)
    /// Cette correspondance est le standard de l'industrie pour équilibrer la densité des marqueurs à l'écran.
    pub fn from_client_zoom(zoom: f32) -> Self {
        let res = match zoom {
            z if z <= 2.0 => 0, // Échelle mondiale (Continent)
            z if z <= 4.0 => 1,
            z if z <= 6.0 => 2,
            z if z <= 8.0 => 3,  // Échelle nationale (ex: la France entière)
            z if z <= 10.0 => 5, // Échelle régionale
            z if z <= 12.0 => 7, // Échelle d'une grande ville / Métropole
            z if z <= 14.0 => 8,
            z if z <= 16.0 => 9, // Échelle d'un quartier dense
            _ => 10,             // Échelle d'une rue / piéton
        };
        Self(res)
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
