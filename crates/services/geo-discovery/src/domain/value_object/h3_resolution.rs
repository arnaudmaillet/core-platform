/// The three H3 resolutions used by this service.
///
/// Every published post is indexed at all three resolutions simultaneously.
/// The resolution selected for a query depends on the client's zoom level.
///
/// | Resolution | Tile area  | Hex diameter | Zoom band   | Virality floor |
/// |------------|------------|--------------|-------------|----------------|
/// | R5         | ~87 km²    | ~400 km      | Zoom 1–4    | 500            |
/// | R7         | ~5.16 km²  | ~36 km       | Zoom 5–12   | 50 / 5         |
/// | R9         | ~0.105 km² | ~3 km        | Zoom 13–15  | 0              |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum H3Resolution {
    R5,
    R7,
    R9,
}

impl H3Resolution {
    pub const ALL: [H3Resolution; 3] = [H3Resolution::R5, H3Resolution::R7, H3Resolution::R9];

    pub fn as_h3o(&self) -> h3o::Resolution {
        match self {
            Self::R5 => h3o::Resolution::Five,
            Self::R7 => h3o::Resolution::Seven,
            Self::R9 => h3o::Resolution::Nine,
        }
    }

    /// Raw tinyint value stored in ScyllaDB and embedded in Redis key suffixes.
    pub fn as_i8(&self) -> i8 {
        match self {
            Self::R5 => 5,
            Self::R7 => 7,
            Self::R9 => 9,
        }
    }

    /// Minimum virality score for posts to appear in a tile query at this resolution.
    pub fn virality_floor(&self, zoom: i32) -> f64 {
        match self {
            Self::R5 => 500.0,
            // Within the R7 band, apply a finer threshold based on zoom.
            Self::R7 if zoom <= 8 => 50.0,
            Self::R7 => 5.0,
            Self::R9 => 0.0,
        }
    }

    /// Maximum number of ZSET members retained per tile (Top-K cap).
    pub fn top_k_cap(&self) -> i64 {
        match self {
            Self::R5 => 200,
            Self::R7 => 500,
            Self::R9 => 1_000,
        }
    }

    /// Approximate hexagon edge length in km. Used for k-ring radius estimation.
    pub fn edge_len_km(&self) -> f64 {
        match self {
            Self::R5 => 252.9,
            Self::R7 => 22.6,
            Self::R9 => 2.0,
        }
    }
}

/// Maps a raw client zoom level (0–15) to an H3 resolution and virality floor.
pub fn zoom_to_resolution(zoom: i32) -> H3Resolution {
    match zoom {
        i32::MIN..=4  => H3Resolution::R5,
        5..=12        => H3Resolution::R7,
        _             => H3Resolution::R9,
    }
}
