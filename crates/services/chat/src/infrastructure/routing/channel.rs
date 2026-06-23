use crate::domain::value_object::ConversationId;

/// Which plane a pub/sub channel belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plane {
    Member,
    Audience,
}

/// Sharded pub/sub channel naming for the two planes.
///
/// Hash-tag placement is the whole point:
/// - **Member** channel `chat:{conv:<id>}:m` carries the `{conv:<id>}` tag, the
///   same slot as the conversation's cache keys — so member messages, presence,
///   typing, and receipts are confined (via `SSUBSCRIBE`) to a single shard, and
///   that high-frequency traffic never crosses to Audience-Plane nodes.
/// - **Audience** channel `chat:{aud:<id>:<k>}` tags each shard distinctly, so
///   shards spread across the cluster and a viral conversation's fan-out is not
///   pinned to one node.
pub struct ChannelScheme;

impl ChannelScheme {
    /// Member-Plane channel for a conversation (single home slot).
    pub fn member_channel(conversation_id: &ConversationId) -> String {
        format!("chat:{{conv:{conversation_id}}}:m")
    }

    /// Audience-Plane channel for a conversation's shard `shard` (spreads).
    pub fn audience_channel(conversation_id: &ConversationId, shard: u16) -> String {
        format!("chat:{{aud:{conversation_id}:{shard}}}")
    }

    /// Recovers `(conversation_id, plane)` from a channel name received on the
    /// subscriber. Returns `None` for an unrecognized channel.
    pub fn parse(channel: &str) -> Option<(ConversationId, Plane)> {
        if let Some(rest) = channel.strip_prefix("chat:{conv:") {
            let id = rest.strip_suffix("}:m")?;
            return ConversationId::try_from(id).ok().map(|c| (c, Plane::Member));
        }
        if let Some(rest) = channel.strip_prefix("chat:{aud:") {
            let inner = rest.strip_suffix('}')?;
            let (id, _shard) = inner.split_once(':')?;
            return ConversationId::try_from(id).ok().map(|c| (c, Plane::Audience));
        }
        None
    }
}
