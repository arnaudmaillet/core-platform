// crates/post/src/domain/types/mentions.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};
use shared_kernel::types::ProfileId;
use std::collections::BTreeSet;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct Mentions(BTreeSet<ProfileId>);

impl Mentions {
    pub const MAX_MENTIONS_COUNT: usize = 20;

    pub fn try_new(profiles: BTreeSet<ProfileId>) -> Result<Self> {
        let mentions = Self(profiles);
        mentions.validate()?;
        Ok(mentions)
    }

    pub fn from_raw(profiles: BTreeSet<ProfileId>) -> Self {
        Self(profiles)
    }

    pub fn empty() -> Self {
        Self(BTreeSet::new())
    }

    pub fn value(&self) -> &BTreeSet<ProfileId> {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn contains(&self, profile_id: &ProfileId) -> bool {
        self.0.contains(profile_id)
    }

    pub fn iter(&self) -> std::collections::btree_set::Iter<'_, ProfileId> {
        self.0.iter()
    }
}

impl ValueObject for Mentions {
    fn validate(&self) -> Result<()> {
        if self.0.len() > Self::MAX_MENTIONS_COUNT {
            return Err(Error::validation(
                "mentions",
                format!(
                    "A single post cannot mention more than {} profiles",
                    Self::MAX_MENTIONS_COUNT
                ),
            ));
        }
        Ok(())
    }
}

// --- CONVERSIONS NATIVES (SERDE & PROTOBUF COMPLIANCE) ---

impl TryFrom<Vec<String>> for Mentions {
    type Error = Error;
    fn try_from(ids: Vec<String>) -> Result<Self> {
        let set: BTreeSet<ProfileId> = ids
            .into_iter()
            .map(|s| ProfileId::from_str(&s))
            .collect::<Result<BTreeSet<ProfileId>>>()?;

        Self::try_new(set)
    }
}

impl From<Mentions> for Vec<String> {
    fn from(mentions: Mentions) -> Self {
        mentions.0.into_iter().map(|id| id.to_string()).collect()
    }
}

impl From<Mentions> for BTreeSet<ProfileId> {
    fn from(mentions: Mentions) -> Self {
        mentions.0
    }
}

impl<'a> IntoIterator for &'a Mentions {
    type Item = &'a ProfileId;
    type IntoIter = std::collections::btree_set::Iter<'a, ProfileId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
