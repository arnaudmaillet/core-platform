// crates/account/src/application/resolve_identity/resolve_identity_use_case.rs (suite)

use crate::application::resolve_identity::{ResolveIdentityCommand, ResolvedIdentityResponse};
use crate::domain::repositories::{AccountMetadataRepository, AccountRepository};
use crate::domain::value_objects::AccountState;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::{DomainError, Result};
use std::sync::Arc;

pub struct ResolveIdentityUseCase {
    account_repo: Arc<dyn AccountRepository>,
    metadata_repo: Arc<dyn AccountMetadataRepository>,
}

impl ResolveIdentityUseCase {
    /// Point d'entrée principal avec Retry pour la résilience aux pannes transitoires
    pub async fn execute(&self, cmd: ResolveIdentityCommand) -> Result<ResolvedIdentityResponse> {
        with_retry(RetryConfig::default(), || async {
            self.try_resolve_once(&cmd).await
        })
        .await
    }

    /// Logique de résolution unique
    async fn try_resolve_once(
        &self,
        cmd: &ResolveIdentityCommand,
    ) -> Result<ResolvedIdentityResponse> {
        // 1. Récupération de l'ID interne (Lookup optimisé)
        let account_id = self
            .account_repo
            .find_account_id_by_external_id(&cmd.external_id, None)
            .await?
            .ok_or_not_found(&cmd.external_id)?;

        // 2. Récupération de l'entité (Vérification d'état)
        let account = self
            .account_repo
            .find_account_by_id(&account_id, None)
            .await?
            .ok_or_not_found(&account_id)?;

        // 3. Fail-Fast : Sécurité
        if account.state().clone() == AccountState::Banned {
            return Err(DomainError::Forbidden {
                reason: "Access denied: This account is permanently banned.".into(),
            });
        }

        // 4. Récupération des métadonnées (Rôles, Beta, etc.)
        let metadata = self
            .metadata_repo
            .find_by_account_id(&account.id())
            .await?
            .ok_or_else(|| {
                DomainError::Internal(format!(
                    "Integrity error: Metadata missing for account {}",
                    account.id()
                ))
            })?;

        // 5. Assemblage
        Ok(ResolvedIdentityResponse {
            account_id: account.id().clone(),
            role: metadata.role(),
            state: account.state().clone(),
            is_beta_tester: metadata.is_beta_tester(),
        })
    }
}
