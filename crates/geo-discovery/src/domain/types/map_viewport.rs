// crates/geo_discovery/src/domain/types/map_viewport.rs

use crate::domain::types::H3Tile;
use h3o::Resolution;
use shared_kernel::core::{Error, Result};
use shared_proto::geo_discovery::v1::LatLng;
use std::str::FromStr;

/// Value Object représentant la zone rectangulaire visible de la carte (Bounding Box).
/// Encapsule les calculs de projection et d'échantillonnage géospatiaux H3.
#[derive(Debug, Clone, PartialEq)]
pub struct MapViewport {
    south_west: LatLng,
    north_east: LatLng,
}

impl MapViewport {
    pub fn try_new(south_west: LatLng, north_east: LatLng) -> Result<Self> {
        if south_west.latitude > north_east.latitude {
            return Err(Error::validation(
                "viewport",
                "La latitude Sud-Ouest ne peut pas être supérieure à la latitude Nord-Est.",
            ));
        }

        Ok(Self {
            south_west,
            north_east,
        })
    }

    pub fn south_west(&self) -> &LatLng {
        &self.south_west
    }

    pub fn north_east(&self) -> &LatLng {
        &self.north_east
    }

    /// Calcule et extrait toutes les cellules H3 uniques intersectant ce Viewport.
    /// La densité du maillage (steps = 10) assure un excellent compromis entre
    /// la couverture sans "trous" visuels et les performances de traitement CPU.
    pub fn get_intersecting_tiles(&self, resolution: Resolution) -> Result<Vec<H3Tile>> {
        let mut tiles = Vec::new();
        let steps = 10;

        let lat_delta = (self.north_east.latitude - self.south_west.latitude) / steps as f64;
        let lng_delta = (self.north_east.longitude - self.south_west.longitude) / steps as f64;

        for i in 0..=steps {
            for j in 0..=steps {
                let lat_deg = self.south_west.latitude + (i as f64 * lat_delta);
                let lng_deg = self.south_west.longitude + (j as f64 * lng_delta);

                // Projection sécurisée en coordonnées H3 (attendant des radians)
                if let Ok(h3_coord) =
                    h3o::LatLng::from_radians(lat_deg.to_radians(), lng_deg.to_radians())
                {
                    let cell_id = h3_coord.to_cell(resolution);

                    if let Ok(tile) = H3Tile::from_str(&cell_id.to_string()) {
                        if !tiles.contains(&tile) {
                            tiles.push(tile);
                        }
                    }
                }
            }
        }

        Ok(tiles)
    }
}
