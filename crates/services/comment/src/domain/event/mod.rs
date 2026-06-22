pub mod comment_created;
pub mod comment_deleted;

pub use comment_created::CommentCreatedEvent;
pub use comment_deleted::CommentDeletedEvent;

#[derive(Debug, Clone)]
pub enum DomainEvent {
    CommentCreated(CommentCreatedEvent),
    CommentDeleted(CommentDeletedEvent),
}
