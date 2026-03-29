// // crates/account/src/infrastructure/api/grpc/handlers/moderation_handler.rs

// use std::sync::Arc;
// use tonic::{Request, Response, Status};
// use shared_proto::account::v1::{
//     account_moderation_service_server::AccountModerationService,
//     BanAccountRequest, UnbanAccountRequest, ShadowbanRequest, 
//     LiftShadowbanRequest, IncreaseTrustScoreRequest, DecreaseTrustScoreRequest,
//     AccountMetadata as ProtoMetadata
// };
// use shared_kernel::domain::value_objects::RegionCode;
// use crate::application::use_cases::{
//     ban_account::*, unban_account::*, shadowban::*, 
//     lift_shadowban::*, increase_trust_score::*, decrease_trust_score::*
// };
// use crate::infrastructure::api::grpc::mappers::errors_mapper::ToGrpcStatus;

// pub struct ModerationHandler {
//     ban_use_case: Arc<BanAccountUseCase>,
//     unban_use_case: Arc<UnbanAccountUseCase>,
//     shadowban_use_case: Arc<ShadowbanUseCase>,
//     lift_shadowban_use_case: Arc<LiftShadowbanUseCase>,
//     increase_trust_use_case: Arc<IncreaseTrustScoreUseCase>,
//     decrease_trust_use_case: Arc<DecreaseTrustScoreUseCase>,
// }

// #[tonic::async_trait]
// impl AccountModerationService for ModerationHandler {
//     async fn ban_account(&self, request: Request<BanAccountRequest>) -> Result<Response<ProtoMetadata>, Status> {
//         let region = self.get_region(&request)?;
//         let cmd = BanAccountCommand::try_from_proto(request.into_inner(), region)?;
//         let res = self.ban_use_case.execute(cmd).await.map_grpc()?;
//         Ok(Response::new(res.into()))
//     }

//     async fn shadowban(&self, request: Request<ShadowbanRequest>) -> Result<Response<ProtoMetadata>, Status> {
//         let region = self.get_region(&request)?;
//         let cmd = ShadowbanCommand::try_from_proto(request.into_inner(), region)?;
//         let res = self.shadowban_use_case.execute(cmd).await.map_grpc()?;
//         Ok(Response::new(res.into()))
//     }

//     async fn increase_trust_score(&self, request: Request<IncreaseTrustScoreRequest>) -> Result<Response<ProtoMetadata>, Status> {
//         let region = self.get_region(&request)?;
//         let cmd = IncreaseTrustScoreCommand::try_from_proto(request.into_inner(), region)?;
//         let res = self.increase_trust_use_case.execute(cmd).await.map_grpc()?;
//         Ok(Response::new(res.into()))
//     }
    
//     // ... implémenter les autres de la même manière ...
// }