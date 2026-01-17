use std::borrow::Cow;
use std::fmt::Debug;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

#[async_trait]
pub trait DomainEvent: Debug + Send + Sync {
    /// Identifiant unique de l'événement (pour l'idempotence)
    fn event_id(&self) -> Uuid {
        Uuid::now_v7()
    }

    /// Nom de l'événement (ex: "user.profile.updated")
    fn event_type(&self) -> Cow<'_, str>;

    /// Nom de l'agrégat (ex: "user")
    fn aggregate_type(&self) -> Cow<'_, str>;

    /// ID de l'agrégat (ex: "123e4567-e89b...")
    fn aggregate_id(&self) -> String;

    /// Horodatage (quand c'est arrivé)
    fn occurred_at(&self) -> DateTime<Utc>;

    /// Les données réelles en JSON
    fn payload(&self) -> Value;

    /// ID de corrélation pour le traçage distribué
    fn correlation_id(&self) -> Option<Uuid> {
        None
    }
}