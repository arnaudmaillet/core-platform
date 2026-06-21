// crates/profile/src/infrastructure/scylla/repositories/scylla_profile_store.rs

use async_trait::async_trait;
use infra_scylla::scylla::{client::session::Session, statement::prepared::PreparedStatement};
use shared_kernel::core::{Error, Identifier, Result, Versioned};
use shared_kernel::types::{AccountId, ProfileId};
use std::any::type_name_of_val;
use std::sync::Arc;

use crate::domain::entities::Profile;
use crate::infrastructure::scylla::mappers::{CqlProfileByAccountRow, CqlProfileRow};
use crate::infrastructure::scylla::statements::{
    DELETE_PROFILE, DELETE_PROFILE_BY_ACCOUNT, FIND_ALL_BY_ACCOUNT_ID, FIND_BY_ID, INSERT_PROFILE,
    INSERT_PROFILE_BY_ACCOUNT, UPDATE_PROFILE,
};
use crate::repositories::ProfileRepository;

pub struct ScyllaProfileStore {
    session: Arc<Session>,
    insert_profile_stmt: PreparedStatement,
    update_profile_stmt: PreparedStatement,
    insert_by_account_stmt: PreparedStatement,
    delete_by_account_stmt: PreparedStatement,
    find_by_id_stmt: PreparedStatement,
    find_all_by_account_stmt: PreparedStatement,
    delete_profile_stmt: PreparedStatement,
}

impl ScyllaProfileStore {
    pub async fn new(session: Arc<Session>, keyspace: String) -> Result<Self> {
        tracing::info!(
            "Preparing Scylla statements for keyspace: {}",
            keyspace.clone()
        );

        let prepare = |template: &'static str, ks: String, sess: Arc<Session>| async move {
            let cql = template.replace("{ks}", &ks);
            tracing::debug!("Preparing CQL: {}", cql);
            sess.prepare(cql).await.map_err(|e| {
                Error::database(format!(
                    "ScyllaDB PreparedStatement failed for keyspace '{}': {}",
                    ks, e
                ))
            })
        };

        let store = Self {
            insert_profile_stmt: prepare(INSERT_PROFILE, keyspace.clone(), session.clone()).await?,
            update_profile_stmt: prepare(UPDATE_PROFILE, keyspace.clone(), session.clone()).await?,
            insert_by_account_stmt: prepare(
                INSERT_PROFILE_BY_ACCOUNT,
                keyspace.clone(),
                session.clone(),
            )
            .await?,
            delete_by_account_stmt: prepare(
                DELETE_PROFILE_BY_ACCOUNT,
                keyspace.clone(),
                session.clone(),
            )
            .await?,
            find_by_id_stmt: prepare(FIND_BY_ID, keyspace.clone(), session.clone()).await?,
            find_all_by_account_stmt: prepare(
                FIND_ALL_BY_ACCOUNT_ID,
                keyspace.clone(),
                session.clone(),
            )
            .await?,
            delete_profile_stmt: prepare(DELETE_PROFILE, keyspace.clone(), session.clone()).await?,
            session,
        };

        tracing::info!(
            "STATEMENT LOG - insert_profile: {:?}",
            store.insert_profile_stmt.get_id()
        );
        tracing::info!(
            "STATEMENT LOG - update_profile: {:?}",
            store.update_profile_stmt.get_id()
        );
        tracing::info!(
            "STATEMENT LOG - insert_by_account: {:?}",
            store.insert_by_account_stmt.get_id()
        );
        Ok(store)
    }
}

#[async_trait]
impl ProfileRepository for ScyllaProfileStore {
    async fn save(&self, profile: &mut Profile) -> Result<()> {
        let row = CqlProfileRow::from_domain(profile);
        let next_version = profile.version() as i64;

        if next_version == 0 {
            let params = (
                row.id,
                row.account_id,
                &row.handle,
                &row.display_name,
                &row.bio,
                &row.avatar_url,
                &row.banner_url,
                &row.location_label,
                &row.social_links,
                row.is_private,
                next_version,
                row.created_at,
                row.updated_at,
            );

            let query_res = self
                .session
                .execute_unpaged(&self.insert_profile_stmt, params)
                .await
                .map_err(|e| Error::database(format!("Failed to insert profile: {}", e)))?;

            // 💡 Extraction déterministe du résultat LWT sans se soucier du nombre de colonnes d'échec
            let rows_res = query_res
                .into_rows_result()
                .map_err(|e| Error::database(e.to_string()))?;
            let mut rows_iter = rows_res
                .rows::<infra_scylla::scylla::value::Row>()
                .map_err(|e| Error::database(e.to_string()))?;

            if let Some(Ok(first_row)) = rows_iter.next() {
                // Dans Scylla, la première colonne d'une LWT est TOUJOURS [applied] (le booléen)
                let applied = first_row.columns[0]
                    .as_ref()
                    .and_then(|c| c.as_boolean())
                    .unwrap_or(false);

                if !applied {
                    return Err(Error::concurrency_conflict(
                        "Profile already exists or race condition occurred during insert"
                            .to_string(),
                    ));
                }
            } else {
                return Err(Error::database(
                    "Empty response from ScyllaDB LWT insert".to_string(),
                ));
            }
        } else {
            let current_version_expected = next_version - 1;
            let params = (
                &row.display_name,
                &row.handle,
                &row.bio,
                &row.avatar_url,
                &row.banner_url,
                &row.location_label,
                &row.social_links,
                row.is_private,
                next_version,
                row.updated_at,
                row.id,
                current_version_expected,
            );

            let query_res = self
                .session
                .execute_unpaged(&self.update_profile_stmt, params)
                .await
                .map_err(|e| Error::database(format!("Failed to update profile: {}", e)))?;

            let rows_res = query_res
                .into_rows_result()
                .map_err(|e| Error::database(e.to_string()))?;

            let mut rows_iter = rows_res
                .rows::<infra_scylla::scylla::value::Row>() // 💡 On utilise le Row brut du driver
                .map_err(|e| Error::database(format!("LWT row parsing failed: {}", e)))?;

            if let Some(Ok(brute_row)) = rows_iter.next() {
                let applied = brute_row.columns[0]
                    .as_ref()
                    .and_then(|c| c.as_boolean())
                    .unwrap_or(false);

                if !applied {
                    return Err(Error::concurrency_conflict(format!(
                        "OCC mismatch for profile {}: database state has changed or version mismatch",
                        row.id
                    )));
                }
            } else {
                return Err(Error::database(
                    "Empty response from ScyllaDB LWT update".to_string(),
                ));
            }
        }

        println!(
            "Types envoyés: {:?}, {:?}, {:?}, {:?}, {:?}, {:?}",
            type_name_of_val(&row.account_id),
            type_name_of_val(&row.id),
            type_name_of_val(&row.handle),
            type_name_of_val(&row.display_name),
            type_name_of_val(&row.avatar_url),
            type_name_of_val(&row.is_private)
        );
        let idx_params = (
            profile.account_id().uuid(),             // account_id
            profile.profile_id().as_uuid(),          // profile_id
            profile.handle().as_str().to_string(),   // handle
            profile.display_name().to_string(),      // display_name
            profile.avatar().map(|u| u.to_string()), // avatar_url (Option<String>)
            profile.is_private(),                    // is_private
        );

        self.session
            .execute_unpaged(&self.insert_by_account_stmt, idx_params)
            .await
            .map_err(|e| Error::database(format!("Secondary index replication failed: {}", e)))?;

        Ok(())
    }

    async fn find_by_id(&self, id: ProfileId) -> Result<Option<Profile>> {
        let res = self
            .session
            .execute_unpaged(&self.find_by_id_stmt, (id.as_uuid(),))
            .await
            .map_err(|e| Error::database(e.to_string()))?;

        let rows = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;

        if let Some(row) = rows
            .maybe_first_row::<CqlProfileRow>()
            .map_err(|e| Error::database(e.to_string()))?
        {
            return Ok(Some(row.to_domain()?));
        }
        Ok(None)
    }

    async fn find_all_by_account_id(&self, account_id: AccountId) -> Result<Vec<Profile>> {
        let res = self
            .session
            .execute_unpaged(&self.find_all_by_account_stmt, (account_id.uuid(),))
            .await
            .map_err(|e| Error::database(e.to_string()))?;

        let rows_res = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;
        let mut rows_iter = rows_res
            .rows::<CqlProfileByAccountRow>()
            .map_err(|e| Error::database(format!("Index mapping failed: {}", e)))?;

        let mut profiles = Vec::new();
        while let Some(row_res) = rows_iter.next() {
            let index_row = row_res.map_err(|e| Error::database(e.to_string()))?;

            if let Some(profile) = self
                .find_by_id(ProfileId::from_uuid(index_row.profile_id))
                .await?
            {
                profiles.push(profile);
            }
        }

        Ok(profiles)
    }

    async fn delete(&self, id: ProfileId) -> Result<()> {
        if let Some(profile) = self.find_by_id(id).await? {
            // 1. Nettoyage de la table secondaire
            self.session
                .execute_unpaged(
                    &self.delete_by_account_stmt,
                    (profile.account_id().uuid(), id.as_uuid()),
                )
                .await
                .map_err(|e| Error::database(format!("Failed to delete secondary index: {}", e)))?;

            // 2. Nettoyage de la table principale
            self.session
                .execute_unpaged(&self.delete_profile_stmt, (id.as_uuid(),))
                .await
                .map_err(|e| Error::database(format!("Failed to delete main profile: {}", e)))?;
        }
        Ok(())
    }

    async fn exists(&self, profile_id: ProfileId) -> Result<bool> {
        let res = self
            .session
            .execute_unpaged(&self.find_by_id_stmt, (profile_id.as_uuid(),))
            .await
            .map_err(|e| Error::database(e.to_string()))?;

        let rows = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;
        Ok(rows.rows_num() > 0)
    }
}
