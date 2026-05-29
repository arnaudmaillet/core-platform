use account::db::PostgresAccountRepository;
use account::entities::Account;
use account::repositories::AccountRepository;
use account::types::{AccountRole, AccountState, RegistrationIdentifier};
use infra_sqlx::PostgresTransaction;
use infra_test::PostgresTestContext;
use tokio;

use shared_kernel::core::{Error, Identifier, Result, Versioned};
use shared_kernel::types::{AccountId, AuditReason, Email, Region, SubId};

async fn get_test_context() -> (PostgresAccountRepository, PostgresTestContext) {
    let pg_ctx = PostgresTestContext::builder()
        .with_migrations(&["./migrations/postgres"])
        .build()
        .await;

    let repo = PostgresAccountRepository::new(pg_ctx.pool().clone());

    (repo, pg_ctx)
}

#[tokio::test]
async fn test_account_full_lifecycle_and_atomicity() -> Result<()> {
    let (repo, pg_ctx) = get_test_context().await;
    let account_id = AccountId::generate();
    let email = Email::try_new("full@lifecycle.com")?;
    let region = Region::default();

    let account = Account::builder(
        account_id,
        RegistrationIdentifier::try_from_email(email.to_string())?,
    )
    .build()?;

    // --- 1. CREATE (Scope isolé) ---
    {
        let tx_sqlx = pg_ctx
            .pool()
            .begin()
            .await
            .map_err(|e| Error::internal(e.to_string()))?;
        let mut tx = PostgresTransaction::new(tx_sqlx);

        repo.create(region, &account, &mut tx).await?;

        tx.into_inner()
            .commit()
            .await
            .map_err(|e| Error::internal(e.to_string()))?;
    }

    // --- 2. FETCH ---
    let found = repo
        .find_by_id(region, account_id, None)
        .await?
        .expect("Account should exist");

    assert_eq!(found.identity().account_id(), account_id);
    assert_eq!(found.version(), 0);

    // --- 3. UPDATE ---
    let mut to_update = found.clone();
    to_update.deactivate(Some(AuditReason::system("deactivate test")))?;
    let _ = to_update.change_role(AccountRole::ADMIN, AuditReason::system("Change Governance"));

    repo.save(region, &mut to_update, None).await?;

    // --- 4. VERIFY UPDATE ---
    let updated = repo.find_by_id(region, account_id, None).await?.unwrap();
    assert_eq!(*updated.identity().state(), AccountState::DEACTIVATED);
    assert_eq!(updated.version(), 1);

    // --- 5. DELETE (Scope isolé) ---
    {
        let mut tx_del = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
        repo.delete(region, account_id, &mut tx_del).await?;
        tx_del.into_inner().commit().await.unwrap();
    }

    let deleted = repo.find_by_id(region, account_id, None).await?;
    assert!(deleted.is_none());

    Ok(())
}

#[tokio::test]
async fn test_concurrency_protection_occ() -> Result<()> {
    let (repo, pg_ctx) = get_test_context().await;
    let account_id = AccountId::generate();
    let region = Region::default();
    let account = Account::builder(
        account_id,
        RegistrationIdentifier::try_from_email("occ@test.com")?,
    )
    .build()?;

    let mut tx = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    repo.create(region, &account, &mut tx).await?;
    tx.into_inner().commit().await.unwrap();

    let mut client_a = repo
        .find_by_id(region, account.account_id(), None)
        .await?
        .unwrap();
    let mut client_b = repo
        .find_by_id(region, account.account_id(), None)
        .await?
        .unwrap();

    client_a.activate()?;
    repo.save(region, &mut client_a, None).await?; // SQL: WHERE version = 0. OK.
    client_b.deactivate(None)?;
    let result = repo.save(region, &mut client_b, None).await;

    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_unique_constraints() -> Result<()> {
    let (repo, pg_ctx) = get_test_context().await;
    let identifier = RegistrationIdentifier::try_from_email("unique@test.com")?;
    let region = Region::default();

    let acc1 = Account::builder(AccountId::generate(), identifier.clone()).build()?;
    let acc2 = Account::builder(AccountId::generate(), identifier.clone()).build()?;

    let mut tx1 = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    repo.create(region, &acc1, &mut tx1).await?;
    tx1.into_inner().commit().await.unwrap();

    // Tentative de doublon
    let mut tx2 = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    let result = repo.create(region, &acc2, &mut tx2).await;

    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn test_lookups() -> Result<()> {
    let (repo, pg_ctx) = get_test_context().await;
    let email = Email::try_new("lookup@test.com")?;
    let region = Region::default();
    let identifier = RegistrationIdentifier::try_from_email(email.to_string())?;
    let ext_id = SubId::from_raw("ext_123");
    let account_id = AccountId::generate();

    let account = Account::builder(account_id, identifier)
        .with_sub_id(ext_id.clone())
        .with_email(email.clone())
        .build()?;

    let mut tx = PostgresTransaction::new(pg_ctx.pool().begin().await.unwrap());
    repo.create(region, &account, &mut tx).await?;
    tx.into_inner().commit().await.unwrap();

    assert!(repo.exists_by_email(region, &email, None).await?);
    assert!(repo.exists_by_sub_id(region, &ext_id, None).await?);

    assert_eq!(
        repo.find_id_by_email(region, &email, None).await?.unwrap(),
        account_id
    );
    assert_eq!(
        repo.find_id_by_sub_id(region, &ext_id, None)
            .await?
            .unwrap(),
        account_id
    );

    Ok(())
}

#[tokio::test]
async fn test_rollback_works_properly() -> Result<()> {
    let (repo, pg_ctx) = get_test_context().await;
    let account_id = AccountId::generate();
    let region = Region::default();
    let account = Account::builder(
        account_id,
        RegistrationIdentifier::try_from_email("rollback@test.com")?,
    )
    .build()?;

    let tx_sqlx = pg_ctx.pool().begin().await.unwrap();
    let mut tx = PostgresTransaction::new(tx_sqlx);

    repo.create(region, &account, &mut tx).await?;
    tx.into_inner().rollback().await.unwrap();

    let found = repo.find_by_id(region, account_id, None).await?;
    assert!(found.is_none(), "Account should not exist after rollback");

    Ok(())
}

#[tokio::test]
async fn test_rigorous_partial_fetch_integrity() -> Result<()> {
    let (repo, pg_ctx) = get_test_context().await;
    let account_id = AccountId::generate();
    let region = Region::default();
    let email = Email::try_new("partial@integrity.com")?;

    // --- 1. INSERTION MANUELLE PARTIELLE (SIMULATION BUG/LATENCE) ---
    // On n'insère QUE l'identité, pas les settings ni la gouvernance.
    sqlx::query("INSERT INTO account_identity (account_id, region, email, locale, state, version, created_at, updated_at, aggregate_updated_at) 
                 VALUES ($1, $2, $3, 'fr-FR', 'ACTIVE', 0, NOW(), NOW(), NOW())")
        .bind(account_id.as_uuid())
        .bind(region.to_string())
        .bind(email.to_string())
        .execute(&pg_ctx.pool())
        .await
        .unwrap();

    // --- 2. TENTATIVE DE FETCH DE L'AGRÉGAT COMPLET ---
    let result = repo.find_by_id(region, account_id, None).await?;
    assert!(
        result.is_some(),
        "Le repo devrait être capable de reconstruire un compte même si les tables satellites sont vides (Audit: Résilience)"
    );

    let account = result.unwrap();
    assert_eq!(
        account.settings().timezone().to_string(),
        "UTC",
        "Devrait avoir une timezone par défaut"
    );

    Ok(())
}
