// backend/services/identity/src/main.rs

use tonic::{transport::Server, Response, Status};

use identity_proto::identity::*;  // Tout le package v1
use identity_proto::identity::user_service_server::{UserService, UserServiceServer};

#[derive(Default)]
pub struct IdentityServiceImpl {}

#[tonic::async_trait]
impl UserService for IdentityServiceImpl {
    async fn get_user(
        &self,
        request: tonic::Request<GetUserRequest>,
    ) -> Result<tonic::Response<GetUserResponse>, Status> {
        let user_id = request.into_inner().user_id;

        println!("🚀 GetUser called for ID: {}", user_id);

        let user = User {
            user_id: user_id.clone(),
            username: format!("user_{}", user_id),
            display_name: "John Doe".to_string(),
            bio: "Ingénieur Core-Platform".to_string(),
            avatar_url: "https://api.dicebear.com/7.x/avataaars/svg?seed=john".to_string(),
            created_at: chrono::Utc::now().timestamp(),
        };

        Ok(Response::new(GetUserResponse { user: Some(user) }))
    }

    async fn get_user_by_username(
        &self,
        request: tonic::Request<GetUserByUsernameRequest>,
    ) -> Result<tonic::Response<GetUserResponse>, Status> {
        let username = request.into_inner().username;

        println!("🚀 GetUserByUsername called for username: {}", username);

        let user = User {
            user_id: "42".to_string(),
            username: username.clone(),
            display_name: "Alice Wonder".to_string(),
            bio: "Développeuse passionnée".to_string(),
            avatar_url: "https://api.dicebear.com/7.x/avataaars/svg?seed=alice".to_string(),
            created_at: chrono::Utc::now().timestamp(),
        };

        Ok(Response::new(GetUserResponse { user: Some(user) }))
    }

    async fn create_user(
        &self,
        request: tonic::Request<CreateUserRequest>,
    ) -> Result<tonic::Response<CreateUserResponse>, Status> {
        let req = request.into_inner();

        println!("🚀 CreateUser called for username: {}", req.username);

        // TODO: hashing du password avec argon2 + insertion en DB

        let new_user = User {
            user_id: "12345".to_string(),  // TODO: générer un vrai UUID
            username: req.username,
            display_name: req.display_name.unwrap_or_else(|| "New User".to_string()),
            bio: "Bienvenue sur Core Platform !".to_string(),
            avatar_url: "https://api.dicebear.com/7.x/avataaars/svg?seed=new".to_string(),
            created_at: chrono::Utc::now().timestamp(),
        };

        Ok(Response::new(CreateUserResponse { user: Some(new_user) }))
    }

    async fn update_profile(
        &self,
        request: tonic::Request<UpdateProfileRequest>,
    ) -> Result<tonic::Response<UpdateProfileResponse>, Status> {
        let req = request.into_inner();

        println!("🚀 UpdateProfile called for user_id: {}", req.user_id);

        // TODO: récupérer l'utilisateur en DB, appliquer les champs optionnels

        let updated_user = User {
            user_id: req.user_id,
            username: "existing_user".to_string(),
            display_name: req.display_name.unwrap_or("Updated Name".to_string()),
            bio: req.bio.unwrap_or("Bio mise à jour".to_string()),
            avatar_url: req.avatar_url.unwrap_or("https://api.dicebear.com/7.x/avataaars/svg?seed=updated".to_string()),
            created_at: 1700000000,
        };

        Ok(Response::new(UpdateProfileResponse { user: Some(updated_user) }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let service = IdentityServiceImpl::default();

    println!("✅ Identity Service listening on {}", addr);

    Server::builder()
        .add_service(UserServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}