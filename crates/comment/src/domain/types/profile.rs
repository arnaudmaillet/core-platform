// crates/content_comments/src/domain/read_models/comment_profile.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::ProfileId;
use std::fmt;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CommentUserProfile {
    profile_id: ProfileId,
    username: String,
    display_name: String,
    avatar_url: Option<String>,
}

impl CommentUserProfile {
    pub fn new(
        profile_id: ProfileId,
        username: String,
        display_name: String,
        avatar_url: Option<String>,
    ) -> Result<Self> {
        if username.trim().is_empty() {
            return Err(Error::validation(
                "username",
                "Le username ne peut pas être vide",
            ));
        }
        if display_name.trim().is_empty() {
            return Err(Error::validation(
                "display_name",
                "Le display_name ne peut pas être vide",
            ));
        }

        let avatar_url = avatar_url.filter(|url| !url.trim().is_empty());

        Ok(Self {
            profile_id,
            username: username.trim().to_string(),
            display_name: display_name.trim().to_string(),
            avatar_url,
        })
    }

    // --- Getters Publics ---

    pub fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn avatar_url(&self) -> Option<&str> {
        self.avatar_url.as_deref()
    }
}

impl fmt::Display for CommentUserProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (@{})", self.display_name, self.username)
    }
}

// Permet de convertir facilement le type généré par ton client gRPC Account
// vers ton Read Model local de manière sécurisée.
// impl TryFrom<shared_proto::account::v1::UserProfileResponse> for CommentUserProfile {
//     type Error = Error;

//     fn try_from(proto: shared_proto::account::v1::UserProfileResponse) -> Result<Self> {
//         let profile_id = ProfileId::try_from(proto.profile_id)?;

//         Self::new(
//             profile_id,
//             proto.username,
//             proto.display_name,
//             proto.avatar_url,
//         )
//     }
// }
