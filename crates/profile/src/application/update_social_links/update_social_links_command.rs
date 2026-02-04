// crates/profile/src/application/use_cases/update_social_links/update_social_links_command.rs

use crate::domain::value_objects::SocialLinks;
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSocialLinksCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_links: Option<SocialLinks>,
}
