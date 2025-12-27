// backend/services/identity/src/main.rs

use tonic::{transport::Server, Request, Response, Status};

// On importe le code généré
use identity_rust_proto::identity::v1 as proto;
use proto::user_service_server::{UserService, UserServiceServer};
use proto::{GetUserResponse, GetUserRequest};

#[derive(Default)]
pub struct IdentityServiceImpl {}

#[tonic::async_trait]
impl UserService for IdentityServiceImpl {
    // Utiliser explicitement les types de tonic garantit la compatibilité
    async fn get_user(
        &self,
        request: tonic::Request<proto::GetUserRequest>,
    ) -> Result<tonic::Response<proto::GetUserResponse>, tonic::Status> {
        let user_id = request.into_inner().user_id;

        println!("🚀 Identity: Request for ID {}", user_id);

        let reply = GetUserResponse {
            username: format!("User_{}", user_id),
            bio: "Ingénieur Core-Platform".into(),
            avatar_url: "https://api.dicebear.com/7.x/avataaars/svg".into(),
        };

        Ok(Response::new(reply))
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