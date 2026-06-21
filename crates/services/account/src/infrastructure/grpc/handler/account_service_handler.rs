use chrono::{DateTime, Utc};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use cqrs::{CommandBus, Envelope, QueryBus};

use crate::application::command::{
    adjust_credit_balance::AdjustCreditBalanceCommand,
    anonymize_account::AnonymizeAccountCommand,
    assign_role::AssignRoleCommand,
    change_password::ChangePasswordCommand,
    create_account::CreateAccountCommand,
    deactivate_account::DeactivateAccountCommand,
    enroll_mfa::EnrollMfaCommand,
    reactivate_account::ReactivateAccountCommand,
    record_failed_login::RecordFailedLoginCommand,
    record_login::RecordLoginCommand,
    request_data_export::RequestDataExportCommand,
    request_gdpr_deletion::RequestGdprDeletionCommand,
    revoke_mfa::RevokeMfaCommand,
    revoke_role::RevokeRoleCommand,
    suspend_account::SuspendAccountCommand,
    update_kyc_status::UpdateKycStatusCommand,
    verify_email::VerifyEmailCommand,
    verify_phone::VerifyPhoneCommand,
};
use crate::application::query::{
    get_account_by_id::{AccountView, GetAccountByIdQuery},
    get_account_by_identity_id::GetAccountByIdentityIdQuery,
    get_account_status::{AccountStatusView, GetAccountStatusQuery},
    get_credit_balance::{CreditBalanceView, GetCreditBalanceQuery},
    get_gdpr_record::{GdprRecordView, GetGdprRecordQuery},
    list_accounts_by_status::{AccountListView, ListAccountsByStatusQuery},
};
use crate::error::AccountError;

// ── Proto inclusion ───────────────────────────────────────────────────────────
pub mod proto {
    tonic::include_proto!("account.v1");
}

pub use proto::account_service_server::AccountServiceServer;

/// gRPC request handler for the Account service.
///
/// Converts every inbound Protobuf request into a CQRS `Envelope<Command>` or
/// `Envelope<Query>`, dispatches it through the respective bus, and converts
/// the result back to a Protobuf response.
pub struct AccountServiceHandler<CB, QB>
where
    CB: CommandBus + Send + Sync + 'static,
    QB: QueryBus + Send + Sync + 'static,
{
    command_bus: CB,
    query_bus: QB,
}

impl<CB, QB> AccountServiceHandler<CB, QB>
where
    CB: CommandBus + Send + Sync + 'static,
    QB: QueryBus + Send + Sync + 'static,
{
    pub fn new(command_bus: CB, query_bus: QB) -> Self {
        Self { command_bus, query_bus }
    }
}

// ── Command handlers ──────────────────────────────────────────────────────────

impl<CB, QB> AccountServiceHandler<CB, QB>
where
    CB: CommandBus + Send + Sync + 'static,
    QB: QueryBus + Send + Sync + 'static,
{
    fn ok_command(account_id: &str) -> Response<proto::CommandResponse> {
        Response::new(proto::CommandResponse {
            success: true,
            account_id: account_id.to_owned(),
        })
    }

    pub async fn create_account(
        &self,
        request: Request<proto::CreateAccountRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let correlation_id = Uuid::now_v7();

        let cmd = CreateAccountCommand {
            identity_id: req.identity_id.clone(),
            email: req.email,
            phone: Some(req.phone).filter(|s| !s.is_empty()),
            password_hash: Some(req.password_hash).filter(|s| !s.is_empty()),
            country_of_residence: Some(req.country_of_residence).filter(|s| !s.is_empty()),
            currency: None,
            role: None,
            created_by: Some(req.created_by).filter(|s| !s.is_empty()),
        };
        self.command_bus
            .dispatch(Envelope::new(correlation_id, cmd))
            .await
            .map(|_| Self::ok_command(&req.identity_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn verify_email(
        &self,
        request: Request<proto::VerifyEmailRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = VerifyEmailCommand { account_id: req.account_id.clone() };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn verify_phone(
        &self,
        request: Request<proto::VerifyPhoneRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = VerifyPhoneCommand { account_id: req.account_id.clone() };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn change_password(
        &self,
        request: Request<proto::ChangePasswordRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = ChangePasswordCommand {
            account_id: req.account_id.clone(),
            new_password_hash: req.new_password_hash,
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn enroll_mfa(
        &self,
        request: Request<proto::EnrollMfaRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = EnrollMfaCommand {
            account_id: req.account_id.clone(),
            totp_secret_ciphertext: req.totp_secret,
            // Recovery codes are generated server-side; none are sent via gRPC in this flow.
            recovery_code_hashes: Vec::new(),
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn revoke_mfa(
        &self,
        request: Request<proto::RevokeMfaRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = RevokeMfaCommand { account_id: req.account_id.clone() };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn update_kyc_status(
        &self,
        request: Request<proto::UpdateKycStatusRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let new_status = kyc_status_i32_to_str(req.kyc_status)
            .ok_or_else(|| Status::invalid_argument("unknown kyc_status value"))?;
        let cmd = UpdateKycStatusCommand {
            account_id: req.account_id.clone(),
            new_status: new_status.to_owned(),
            reviewer_id: req.reviewer_id,
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn suspend_account(
        &self,
        request: Request<proto::SuspendAccountRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = SuspendAccountCommand {
            account_id: req.account_id.clone(),
            reason: req.reason,
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn reactivate_account(
        &self,
        request: Request<proto::ReactivateAccountRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = ReactivateAccountCommand { account_id: req.account_id.clone() };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn deactivate_account(
        &self,
        request: Request<proto::DeactivateAccountRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = DeactivateAccountCommand { account_id: req.account_id.clone() };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn record_login(
        &self,
        request: Request<proto::RecordLoginRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = RecordLoginCommand { account_id: req.account_id.clone() };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn record_failed_login(
        &self,
        request: Request<proto::RecordFailedLoginRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = RecordFailedLoginCommand {
            account_id: req.account_id.clone(),
            max_attempts: 5,           // default; can be made configurable
            lockout_duration_secs: 900,// default: 15 minutes
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn request_gdpr_deletion(
        &self,
        request: Request<proto::RequestGdprDeletionRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = RequestGdprDeletionCommand {
            account_id: req.account_id.clone(),
            retention_days: 30, // default legal retention; can be made configurable
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn anonymize_account(
        &self,
        request: Request<proto::AnonymizeAccountRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = AnonymizeAccountCommand { account_id: req.account_id.clone() };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn request_data_export(
        &self,
        request: Request<proto::RequestDataExportRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = RequestDataExportCommand { account_id: req.account_id.clone() };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn adjust_credit_balance(
        &self,
        request: Request<proto::AdjustCreditBalanceRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let cmd = AdjustCreditBalanceCommand {
            account_id: req.account_id.clone(),
            delta: req.delta,
            currency: req.currency,
            transaction_id: req.transaction_id,
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn assign_role(
        &self,
        request: Request<proto::AssignRoleRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let role = account_role_i32_to_str(req.role)
            .ok_or_else(|| Status::invalid_argument("unknown role value"))?;
        let cmd = AssignRoleCommand {
            account_id: req.account_id.clone(),
            role: role.to_owned(),
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }

    pub async fn revoke_role(
        &self,
        request: Request<proto::RevokeRoleRequest>,
    ) -> Result<Response<proto::CommandResponse>, Status> {
        let req = request.into_inner();
        let role = account_role_i32_to_str(req.role)
            .ok_or_else(|| Status::invalid_argument("unknown role value"))?;
        let cmd = RevokeRoleCommand {
            account_id: req.account_id.clone(),
            role: role.to_owned(),
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .map(|_| Self::ok_command(&req.account_id))
            .map_err(cqrs_error_to_status)
    }
}

// ── Query handlers ────────────────────────────────────────────────────────────

impl<CB, QB> AccountServiceHandler<CB, QB>
where
    CB: CommandBus + Send + Sync + 'static,
    QB: QueryBus + Send + Sync + 'static,
{
    pub async fn get_account_by_id(
        &self,
        request: Request<proto::GetAccountByIdRequest>,
    ) -> Result<Response<proto::AccountView>, Status> {
        let req = request.into_inner();
        let query = GetAccountByIdQuery { account_id: req.account_id };
        let view: AccountView = self
            .query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), query))
            .await
            .map_err(cqrs_error_to_status)?;
        Ok(Response::new(account_view_to_proto(view)))
    }

    pub async fn get_account_by_identity_id(
        &self,
        request: Request<proto::GetAccountByIdentityIdRequest>,
    ) -> Result<Response<proto::AccountView>, Status> {
        let req = request.into_inner();
        let query = GetAccountByIdentityIdQuery { identity_id: req.identity_id };
        let view: AccountView = self
            .query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), query))
            .await
            .map_err(cqrs_error_to_status)?;
        Ok(Response::new(account_view_to_proto(view)))
    }

    pub async fn get_account_status(
        &self,
        request: Request<proto::GetAccountStatusRequest>,
    ) -> Result<Response<proto::AccountStatusView>, Status> {
        let req = request.into_inner();
        let query = GetAccountStatusQuery { account_id: req.account_id };
        let view: AccountStatusView = self
            .query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), query))
            .await
            .map_err(cqrs_error_to_status)?;
        Ok(Response::new(proto::AccountStatusView {
            account_id: view.account_id,
            status: account_status_str_to_i32(view.status.as_str()),
            suspension_reason: view.suspension_reason.unwrap_or_default(),
        }))
    }

    pub async fn get_credit_balance(
        &self,
        request: Request<proto::GetCreditBalanceRequest>,
    ) -> Result<Response<proto::CreditBalanceView>, Status> {
        let req = request.into_inner();
        let query = GetCreditBalanceQuery { account_id: req.account_id };
        let view: CreditBalanceView = self
            .query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), query))
            .await
            .map_err(cqrs_error_to_status)?;
        Ok(Response::new(proto::CreditBalanceView {
            account_id: view.account_id,
            balance: view.balance,
            reserved: view.reserved,
            currency: view.currency.unwrap_or_default(),
            ledger_version: view.ledger_version,
        }))
    }

    pub async fn get_gdpr_record(
        &self,
        request: Request<proto::GetGdprRecordRequest>,
    ) -> Result<Response<proto::GdprRecordView>, Status> {
        let req = request.into_inner();
        let query = GetGdprRecordQuery { account_id: req.account_id };
        let view: GdprRecordView = self
            .query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), query))
            .await
            .map_err(cqrs_error_to_status)?;
        Ok(Response::new(proto::GdprRecordView {
            account_id: view.account_id,
            data_processing_consented: view.data_processing_consented_at.is_some(),
            marketing_consented: view.marketing_consented_at.is_some(),
            deletion_requested_at: view.deletion_requested_at.map(dt_to_ts),
            anonymized_at: view.anonymized_at.map(dt_to_ts),
            data_export_requested_at: view.data_export_requested_at.map(dt_to_ts),
            data_export_completed_at: view.data_export_completed_at.map(dt_to_ts),
        }))
    }

    pub async fn list_accounts_by_status(
        &self,
        request: Request<proto::ListAccountsByStatusRequest>,
    ) -> Result<Response<proto::ListAccountsByStatusResponse>, Status> {
        let req = request.into_inner();
        let status = account_status_i32_to_str(req.status)
            .ok_or_else(|| Status::invalid_argument("invalid status value"))?;
        let query = ListAccountsByStatusQuery {
            status: status.to_owned(),
            limit: req.limit.max(1).min(1000) as i64,
            offset: req.offset.max(0) as i64,
        };
        let view: AccountListView = self
            .query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), query))
            .await
            .map_err(cqrs_error_to_status)?;
        Ok(Response::new(proto::ListAccountsByStatusResponse {
            accounts: view.accounts.into_iter().map(account_view_to_proto).collect(),
            total: view.total,
        }))
    }
}

// ── Proto conversion helpers ──────────────────────────────────────────────────

fn dt_to_ts(dt: DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

fn account_view_to_proto(v: AccountView) -> proto::AccountView {
    proto::AccountView {
        id: v.id,
        identity_id: v.identity_id,
        status: account_status_str_to_i32(&v.status),
        suspension_reason: v.suspension_reason.unwrap_or_default(),
        email: v.email,
        email_verified: v.email_verified,
        phone: v.phone.unwrap_or_default(),
        phone_verified: v.phone_verified,
        kyc_status: kyc_status_str_to_i32(&v.kyc_status),
        country_of_residence: v.country_of_residence.unwrap_or_default(),
        roles: v.roles,
        credit_balance: v.credit_balance,
        credit_reserved: v.credit_reserved,
        credit_currency: v.credit_currency.unwrap_or_default(),
        version: v.version,
        created_at: Some(dt_to_ts(v.created_at)),
        updated_at: Some(dt_to_ts(v.updated_at)),
    }
}

fn account_status_str_to_i32(s: &str) -> i32 {
    match s {
        "pending_verification" => 1,
        "active"               => 2,
        "suspended"            => 3,
        "deactivated"          => 4,
        "deleted"              => 5,
        _                      => 0,
    }
}

fn account_status_i32_to_str(v: i32) -> Option<&'static str> {
    match v {
        1 => Some("pending_verification"),
        2 => Some("active"),
        3 => Some("suspended"),
        4 => Some("deactivated"),
        5 => Some("deleted"),
        _ => None,
    }
}

fn kyc_status_str_to_i32(s: &str) -> i32 {
    match s {
        "not_started" => 1,
        "submitted"   => 2,
        "in_review"   => 3,
        "approved"    => 4,
        "rejected"    => 5,
        _             => 0,
    }
}

fn kyc_status_i32_to_str(v: i32) -> Option<&'static str> {
    match v {
        1 => Some("not_started"),
        2 => Some("submitted"),
        3 => Some("in_review"),
        4 => Some("approved"),
        5 => Some("rejected"),
        _ => None,
    }
}

fn account_role_i32_to_str(v: i32) -> Option<&'static str> {
    match v {
        1 => Some("user"),
        2 => Some("content_moderator"),
        3 => Some("support_agent"),
        4 => Some("finance_operator"),
        5 => Some("admin"),
        6 => Some("super_admin"),
        _ => None,
    }
}

// ── Error mapping ─────────────────────────────────────────────────────────────

pub fn cqrs_error_to_status(err: cqrs::error::CqrsError) -> Status {
    use cqrs::error::CqrsError;
    match err {
        CqrsError::HandlerNotFound { type_name } => {
            Status::unimplemented(format!("no handler registered for {type_name}"))
        }
        CqrsError::DuplicateRegistration { type_name } => {
            Status::internal(format!("duplicate handler for {type_name}"))
        }
        CqrsError::Handler(boxed) => {
            // BoxedDynAppError has no downcast path; map via AppError trait metadata.
            use error::AppError as _;
            let msg = boxed.to_string();
            let retryable = boxed.is_retryable();
            match boxed.http_status().as_u16() {
                404 => Status::not_found(msg),
                409 if retryable => Status::aborted(msg),
                409 => Status::already_exists(msg),
                400 | 422 => Status::failed_precondition(msg),
                503 | 502 => Status::unavailable(msg),
                _ => Status::internal(msg),
            }
        }
    }
}

pub fn account_error_to_status(err: &AccountError) -> Status {
    match err {
        AccountError::AccountNotFound { id } => {
            Status::not_found(format!("account not found: {id}"))
        }
        AccountError::IdentityAlreadyRegistered { identity_id } => {
            Status::already_exists(format!("identity '{identity_id}' is already registered"))
        }
        AccountError::EmailAlreadyRegistered { email } => {
            Status::already_exists(format!("email '{email}' is already registered"))
        }
        AccountError::ConcurrentModification => {
            Status::aborted("concurrent modification — please retry")
        }
        AccountError::AccountNotActive { current } => {
            Status::failed_precondition(format!("account is not active (current: {current})"))
        }
        AccountError::InvalidStatusTransition { from, to } => {
            Status::failed_precondition(format!("cannot transition from {from} to {to}"))
        }
        AccountError::InvalidKycTransition { from, to } => {
            Status::failed_precondition(format!("KYC cannot transition from {from} to {to}"))
        }
        AccountError::InvalidKycStatus(s) => {
            Status::invalid_argument(format!("invalid KYC status: {s}"))
        }
        AccountError::InvalidAccountRole(r) => {
            Status::invalid_argument(format!("invalid account role: {r}"))
        }
        AccountError::GdprDeletionAlreadyRequested => {
            Status::already_exists("GDPR deletion already requested")
        }
        AccountError::AccountAlreadyAnonymized => {
            Status::failed_precondition("account has already been anonymized")
        }
        AccountError::InsufficientBalance => {
            Status::failed_precondition("insufficient credit balance")
        }
        AccountError::CurrencyMismatch { account, requested } => {
            Status::failed_precondition(format!(
                "currency mismatch: account={account}, requested={requested}"
            ))
        }
        AccountError::MfaAlreadyEnrolled => {
            Status::already_exists("MFA is already enrolled")
        }
        AccountError::MfaNotEnrolled => {
            Status::failed_precondition("MFA is not enrolled")
        }
        AccountError::RoleAlreadyAssigned(r) => {
            Status::already_exists(format!("role {r} is already assigned"))
        }
        AccountError::RoleNotAssigned(r) => {
            Status::not_found(format!("role {r} is not assigned"))
        }
        AccountError::EmailAlreadyVerified => {
            Status::already_exists("email is already verified")
        }
        AccountError::LedgerCurrencyNotSet => {
            Status::failed_precondition("ledger currency is not set")
        }
        AccountError::ArithmeticOverflow => {
            Status::internal("arithmetic overflow in credit calculation")
        }
        AccountError::Validation(e) => {
            Status::invalid_argument(e.to_string())
        }
        AccountError::Storage(_) => {
            Status::unavailable("a database error occurred — please retry")
        }
        _ => Status::internal("an unexpected error occurred"),
    }
}
