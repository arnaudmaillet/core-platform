// crates/profile/src/infrastructure/scylla/repositories/scylla_profile_tests.rs

use profile::entities::Profile;
use profile::repositories::ProfileRepository;
use profile::stores::ScyllaProfileStore;
use profile::types::{DisplayName, Handle};
use tokio;

use infra_test::ScyllaTestContext; // Ton environnement de test ScyllaDB
use shared_kernel::core::{ErrorCode, Identifier, Result, Versioned};
use shared_kernel::types::{AccountId, ProfileId, Region};

async fn get_test_context() -> (ScyllaProfileStore, ScyllaTestContext) {
    let scylla_ctx = ScyllaTestContext::builder()
        .with_migrations(&["./migrations/scylla"])
        .build()
        .await;

    // Le store de production Scylla est instancié pour une région spécifique (silo local)
    let region = Region::default();
    let repo = ScyllaProfileStore::new(scylla_ctx.session().clone(), region)
        .await
        .expect("Failed to initialize ScyllaProfileStore for testing");

    (repo, scylla_ctx)
}

#[tokio::test]
async fn test_profile_full_lifecycle_and_atomicity() -> Result<()> {
    // --- Arrange ---
    let (repo, _scylla_ctx) = get_test_context().await;
    let account_id = AccountId::generate();
    let profile_id = ProfileId::generate();
    let handle = Handle::try_new("alice_rocks")?;
    let mut profile = Profile::builder(account_id, profile_id, handle)?.build()?;

    // --- Act: Étape 1 (Sauvegarde Initiale via LWT) ---
    repo.save(&mut profile).await?;

    // --- Assert: Étape 2 (Vérification de l'état initial) ---
    let found = repo
        .find_by_id(profile_id)
        .await?
        .expect("Profile should exist after initial save");

    assert_eq!(found.version(), 0); // Version initiale de l'agrégat

    // --- Act: Étape 3 (Mise à jour avec la logique de Concurrence Optimiste) ---
    let mut to_update = found.clone();
    to_update.update_display_name(DisplayName::try_new("Alice In Wonderland")?)?;
    // Le domaine incrémente la version : v0 -> v1

    repo.save(&mut to_update).await?;

    // --- Assert: Étape 4 (Vérification de l'état mis à jour) ---
    let updated = repo
        .find_by_id(profile_id)
        .await?
        .expect("Profile should exist after update");

    assert_eq!(updated.display_name().as_str(), "Alice In Wonderland");
    assert_eq!(updated.version(), 1); // Version incrémentée persistée

    // --- Act: Étape 5 (Suppression définitive principale + index de compte) ---
    repo.delete(profile_id).await?;

    // --- Assert: État final ---
    let deleted = repo.find_by_id(profile_id).await?;
    assert!(deleted.is_none(), "Profile should be null after deletion");

    Ok(())
}

#[tokio::test]
async fn test_profile_concurrency_protection_occ() -> Result<()> {
    // --- Arrange ---
    let (repo, _scylla_ctx) = get_test_context().await;
    let account_id = AccountId::generate();
    let profile_id = ProfileId::generate();
    let mut profile =
        Profile::builder(account_id, profile_id, Handle::try_new("ConcurrentUser")?)?.build()?;

    repo.save(&mut profile).await?;

    // Charger deux instances en concurrence au même instant (v0)
    let mut client_a = repo.find_by_id(profile.profile_id()).await?.unwrap();
    let mut client_b = repo.find_by_id(profile.profile_id()).await?.unwrap();

    // Le Client A gagne la course d'écriture
    client_a.update_display_name(DisplayName::try_new("Winner")?)?;
    repo.save(&mut client_a).await?; // LWT applique le changement et passe à la v1 en DB

    // Le Client B tente de sauvegarder sa version obsolète (v0 -> v1) mais la DB est déjà en v1
    client_b.update_display_name(DisplayName::try_new("Loser")?)?;
    let result = repo.save(&mut client_b).await;

    // --- Assert ---
    // La clause IF version = :expected de ScyllaDB doit lever un conflit de concurrence
    assert!(matches!(
        result,
        Err(e) if e.code == ErrorCode::ConcurrencyConflict
    ));
    Ok(())
}

#[tokio::test]
async fn test_find_all_by_account_id() -> Result<()> {
    // --- Arrange ---
    let (repo, _scylla_ctx) = get_test_context().await;
    let account_id = AccountId::generate();
    let profile_id = ProfileId::generate();
    let alt_profile_id = ProfileId::generate();

    let mut p1 =
        Profile::builder(account_id, profile_id, Handle::try_new("profile_1")?)?.build()?;
    let mut p2 =
        Profile::builder(account_id, alt_profile_id, Handle::try_new("profile_2")?)?.build()?;

    repo.save(&mut p1).await?;
    repo.save(&mut p2).await?;

    // --- Act ---
    // Interroge la table de requêtage secondaire `profiles_by_account`
    let profiles = repo.find_all_by_account_id(account_id).await?;

    // --- Assert ---
    assert_eq!(profiles.len(), 2);

    // ScyllaDB trie par la clustering key (profile_id), on vérifie la présence des handles
    let handles: Vec<&str> = profiles.iter().map(|p| p.handle().as_str()).collect();
    assert!(handles.contains(&"profile_1"));
    assert!(handles.contains(&"profile_2"));

    Ok(())
}

#[tokio::test]
async fn test_profile_partial_data_resilience() -> Result<()> {
    // --- Arrange ---
    let (repo, scylla_ctx) = get_test_context().await;

    let domain_profile_id = ProfileId::generate();
    let domain_account_id = AccountId::generate();

    let profile_uuid = domain_profile_id.as_uuid();
    let account_uuid = domain_account_id.uuid();

    // On force l'injection d'une ligne brute ScyllaDB sans les champs optionnels (bio, urls, etc.)
    // pour valider la robustesse de notre mapper d'infrastructure (ScyllaProfileRow -> Domain)
    let raw_cql = r#"
        INSERT INTO test_profile_storage.profiles 
        (id, account_id, handle, display_name, is_private, version, created_at, updated_at)
        VALUES (?, ?, ?, ?, false, 1, toTimestamp(now()), toTimestamp(now()))
    "#;

    let region = Region::default();
    let ks = format!("{}_profile_storage", region.to_string().to_lowercase());
    let cql = raw_cql.replace("test_profile_storage", &ks);

    scylla_ctx
        .session()
        .query_unpaged(cql, (profile_uuid, account_uuid, "mini", "Minimalist"))
        .await
        .map_err(|e| shared_kernel::core::Error::database(e.to_string()))?;

    // --- Act ---
    let fetch_res = repo.find_by_id(domain_profile_id).await;

    // --- Assert ---
    let result = match fetch_res {
        Ok(opt) => opt,
        Err(e) => {
            panic!(
                "Le mapper NoSQL a échoué à parser les valeurs NULL (Champs manquants) : {}",
                e.message
            );
        }
    };

    assert!(
        result.is_some(),
        "Le profil aurait dû être hydraté malgré les champs NULL"
    );
    let p = result.unwrap();
    assert_eq!(p.display_name().as_str(), "Minimalist");
    assert!(p.avatar().is_none());
    assert!(p.bio().is_none());

    Ok(())
}
