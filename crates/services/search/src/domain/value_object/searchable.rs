use serde::{Deserialize, Serialize};

/// Whether a document is currently retrievable by search.
///
/// This is the moderation-visibility flag. A `moderation` content-hide flips it to
/// [`Searchable::HIDDEN`] (the document is **retained**, because moderation is
/// reversible); an appeal reversal flips it back to [`Searchable::VISIBLE`]. Every
/// query filters on it. It is distinct from deletion: a hidden doc can come back, a
/// deleted/purged doc is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Searchable(bool);

impl Searchable {
    pub const VISIBLE: Self = Self(true);
    pub const HIDDEN: Self = Self(false);

    pub fn from_bool(visible: bool) -> Self {
        Self(visible)
    }

    pub fn is_visible(&self) -> bool {
        self.0
    }
}

impl Default for Searchable {
    /// Newly-indexed content is visible unless moderation says otherwise.
    fn default() -> Self {
        Searchable::VISIBLE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_and_hidden_are_opposites() {
        assert!(Searchable::VISIBLE.is_visible());
        assert!(!Searchable::HIDDEN.is_visible());
    }

    #[test]
    fn default_is_visible() {
        assert!(Searchable::default().is_visible());
    }
}
