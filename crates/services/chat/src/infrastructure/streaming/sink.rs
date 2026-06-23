use std::sync::Arc;

use crate::domain::value_object::ConversationId;
use crate::infrastructure::routing::{InboundSink, Plane, PlaneEvent};
use crate::infrastructure::streaming::registry::ConversationBroadcastRegistry;

/// Routes inbound plane events from the Redis subscriber to the correct
/// in-process registry. This is the seam named by [`InboundSink`]: the Phase 5
/// subscriber decodes an event and the sink delivers it to either the Member or
/// the Audience registry, never both.
pub struct PlaneFanoutSink {
    member:   Arc<ConversationBroadcastRegistry>,
    audience: Arc<ConversationBroadcastRegistry>,
}

impl PlaneFanoutSink {
    pub fn new(
        member:   Arc<ConversationBroadcastRegistry>,
        audience: Arc<ConversationBroadcastRegistry>,
    ) -> Self {
        Self { member, audience }
    }

    pub fn member(&self) -> &Arc<ConversationBroadcastRegistry> {
        &self.member
    }

    pub fn audience(&self) -> &Arc<ConversationBroadcastRegistry> {
        &self.audience
    }
}

impl InboundSink for PlaneFanoutSink {
    fn deliver(&self, conversation_id: ConversationId, plane: Plane, event: PlaneEvent) {
        let payload = Arc::new(event);
        match plane {
            Plane::Member   => self.member.broadcast(&conversation_id, payload),
            Plane::Audience => self.audience.broadcast(&conversation_id, payload),
        }
    }
}
