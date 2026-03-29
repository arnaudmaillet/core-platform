// crates/account/src/application/resolve_identity/resolve_identity_use_case.rs

use crate::application::use_cases::resolve_identity::{ResolveIdentityCommand, ResolvedIdentityResponse};
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
    pub fn new(
        account_repo: Arc<dyn AccountRepository>,
        metadata_repo: Arc<dyn AccountMetadataRepository>,
    ) -> Self {
        Self {
            account_repo,
            metadata_repo,
        }
    }

    /// Point d'entrée avec Retry pour la résilience
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
        // 1. Récupération de l'ID interne (Lookup indexé ultra-rapide)
        let account_id = self
            .account_repo
            .resolve_id_from_external_id(&cmd.external_id)
            .await?
            .ok_or_not_found(&cmd.external_id)?;

        // 2. Récupération de l'entité (Vérification d'état)
        let account = self
            .account_repo
            .fetch_by_id(&account_id, None)
            .await?
            .ok_or_not_found(&account_id)?;

        // 3. Fail-Fast : Sécurité (Vérifie si le compte est banni)
        if *account.state() == AccountState::Banned {
            return Err(DomainError::Forbidden {
                reason: "Access denied: This account is permanently banned.".into(),
            });
        }

        // 4. Récupération des métadonnées (Rôles, Beta, etc.)
        let metadata = self
            .metadata_repo
            .fetch_by_account_id(&account_id)
            .await?
            .ok_or_else(|| {
                DomainError::Internal(format!(
                    "Integrity error: Metadata missing for account {}",
                    account_id
                ))
            })?;

        // 5. Assemblage de la réponse
        Ok(ResolvedIdentityResponse {
            account_id,
            role: metadata.role(),
            state: account.state().clone(),
            is_beta_tester: metadata.is_beta_tester(),
        })
    }
}