//! Integration harness: boots an ephemeral ScyllaDB container, wires a real
//! comment graph against it through the production composition root, and exposes
//! the buses for assertions. The event publisher is an in-process no-op.
#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use uuid::Uuid;

use cqrs::command::InMemoryCommandBus;
use cqrs::query::InMemoryQueryBus;
use cqrs::{CommandBus, CqrsError, Envelope, QueryBus};
use scylla_storage::ScyllaConfig;

use comment::app::{App, Backends};
use comment::application::command::create_comment::CreateCommentCommand;
use comment::application::command::delete_comment::DeleteCommentCommand;
use comment::application::port::{CommentEventPublisher, CommentSummary};
use comment::application::query::get_comment::GetCommentQuery;
use comment::application::query::list_replies::ListRepliesQuery;
use comment::application::query::list_top_level::ListTopLevelQuery;
use comment::domain::event::DomainEvent;
use comment::error::CommentError;

pub use comment::domain::aggregate::Comment;
pub use comment::domain::value_object::CommentStatus;
pub use test_support::await_until;

/// Generous default patience for a cross-component assertion (ScyllaDB dual-table
/// write visibility).
pub const DEADLINE: Duration = Duration::from_secs(10);

/// ScyllaDB keyspace the migrations provision.
const KEYSPACE: &str = "comment";
/// On-disk migration assets, resolved against *this* crate's manifest.
const MIGRATIONS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations");

/// A no-op event publisher: these scenarios assert on ScyllaDB, not on the Kafka
/// contract.
struct NoopPublisher;

#[async_trait]
impl CommentEventPublisher for NoopPublisher {
    async fn publish(&self, _event: &DomainEvent) -> Result<(), CommentError> {
        Ok(())
    }
}

/// A fully-wired comment service bound to ephemeral infra, plus the buses.
pub struct TestHarness {
    pub command_bus: Arc<InMemoryCommandBus>,
    pub query_bus:   Arc<InMemoryQueryBus>,
}

impl TestHarness {
    /// Boots/reuses the shared ScyllaDB container, applies migrations, and
    /// assembles the service graph with a no-op publisher.
    pub async fn start() -> Self {
        let scylla_cp = test_support::containers::scylla_ready(KEYSPACE, MIGRATIONS_DIR).await;

        let backends = Backends {
            scylla: ScyllaConfig {
                contact_points: vec![scylla_cp],
                keyspace:       None,
                ..ScyllaConfig::default()
            },
        };

        let app = App::build(backends, Arc::new(NoopPublisher))
            .await
            .expect("integration: build comment app");

        Self { command_bus: app.command_bus, query_bus: app.query_bus }
    }

    /// Creates a comment (top-level when `parent` is `None`, else a reply) authored
    /// by `author_id`, returning its id.
    pub async fn create(&self, post_id: &str, parent: Option<&str>, author_id: &str) -> String {
        let comment_id = Uuid::now_v7().to_string();
        let cmd = CreateCommentCommand {
            comment_id: comment_id.clone(),
            post_id:    post_id.to_owned(),
            author_id:  author_id.to_owned(),
            parent_id:  parent.map(str::to_owned),
            body:       Some("a comment".to_owned()),
            gif_id:     None,
            gif_url:    None,
            gif_width:  None,
            gif_height: None,
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .expect("create_comment");
        comment_id
    }

    /// Deletes a comment as `author`.
    pub async fn delete(&self, comment_id: &str, author_id: &str) {
        let cmd = DeleteCommentCommand {
            comment_id: comment_id.to_owned(),
            author_id:  author_id.to_owned(),
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .expect("delete_comment");
    }

    /// Reads a single comment from the `comments` table (`Err` when absent).
    pub async fn get(&self, comment_id: &str) -> Result<Comment, CqrsError> {
        self.query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), GetCommentQuery { comment_id: comment_id.to_owned() }))
            .await
    }

    /// Lists top-level comments of a post from the `comments_by_post` index.
    pub async fn list_top_level(&self, post_id: &str) -> Vec<CommentSummary> {
        let (summaries, _next) = self
            .query_bus
            .dispatch(Envelope::new(
                Uuid::now_v7(),
                ListTopLevelQuery { post_id: post_id.to_owned(), limit: 100, page_token: None },
            ))
            .await
            .expect("list_top_level");
        summaries
    }

    /// Lists the replies to `parent` under `post` from the thread index.
    pub async fn list_replies(&self, post_id: &str, parent: &str) -> Vec<CommentSummary> {
        let (summaries, _next) = self
            .query_bus
            .dispatch(Envelope::new(
                Uuid::now_v7(),
                ListRepliesQuery {
                    post_id:    post_id.to_owned(),
                    comment_id: parent.to_owned(),
                    limit:      100,
                    page_token: None,
                },
            ))
            .await
            .expect("list_replies");
        summaries
    }
}

/// A fresh random post id (UUID string).
pub fn random_post() -> String {
    Uuid::now_v7().to_string()
}

/// A fresh random author/profile id (UUID string).
pub fn random_author() -> String {
    Uuid::now_v7().to_string()
}

/// Whether `summaries` contains a comment with the given id.
pub fn summaries_contain(summaries: &[CommentSummary], comment_id: &str) -> bool {
    summaries.iter().any(|s| s.comment_id.as_str() == comment_id)
}
