use async_trait::async_trait;
use crate::entities::User;
use crate::value_objects::{UserId, Username, Email, PhoneNumber};
use crate::errors::Result;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>>;
    async fn find_by_username(&self, username: &Username) -> Result<Option<User>>;
    async fn find_by_email(&self, email: &Email) -> Result<Option<User>>;
    async fn find_by_phone(&self, phone: &PhoneNumber) -> Result<Option<User>>;
    async fn find_by_cognito_sub(&self, sub: &str) -> Result<Option<User>>;

    async fn insert(&self, user: &User) -> Result<()>;
    async fn update(&self, user: &User) -> Result<()>;
}