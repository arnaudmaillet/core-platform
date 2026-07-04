use crate::error::ChatError;

/// Maximum length of a message body, in Unicode scalar values.
pub const MAX_MESSAGE_CHARS: usize = 4096;

/// Validated textual body of a message.
///
/// Enforces only the length bound here; emptiness rules are content-type
/// dependent and enforced by [`Message::create`](crate::domain::aggregate::Message::create)
/// (a `Text` message must be non-empty; a `Media` caption may be empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageContent(String);

impl MessageContent {
    pub fn new(s: impl Into<String>) -> Result<Self, ChatError> {
        let s = s.into();
        let len = s.chars().count();
        if len > MAX_MESSAGE_CHARS {
            return Err(ChatError::MessageTooLong { max: MAX_MESSAGE_CHARS, got: len });
        }
        Ok(Self(s))
    }

    pub fn empty() -> Self {
        Self(String::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<MessageContent> for String {
    fn from(c: MessageContent) -> Self {
        c.0
    }
}
