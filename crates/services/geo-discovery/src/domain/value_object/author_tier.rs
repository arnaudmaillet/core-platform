/// Tier classification for a post's author.
///
/// Stored as a `tinyint` (i8) in ScyllaDB and as a `u8` in the Redis msgpack
/// card payload. Serialised as a plain integer — no string tag overhead.
///
/// Used by the client exclusively to render static badge decorations on map
/// pins (gold border for VIP, purple for Premium). Dynamic relational state
/// (friend / following) is resolved client-side from the session social graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AuthorTier {
    Standard = 0,
    Premium  = 1,
    Vip      = 2,
}

impl AuthorTier {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Premium,
            2 => Self::Vip,
            _ => Self::Standard,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns the ScyllaDB `tinyint`-compatible signed byte.
    pub fn as_i8(self) -> i8 {
        self as u8 as i8
    }
}

impl From<i8> for AuthorTier {
    fn from(v: i8) -> Self {
        Self::from_u8(v as u8)
    }
}

impl From<AuthorTier> for i8 {
    fn from(t: AuthorTier) -> i8 {
        t.as_i8()
    }
}
