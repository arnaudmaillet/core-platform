-- crates/account/migrations/postgres/0002_global_registry.sql

-- =========================================================================
-- SCHEMA GLOBAL : UNICITÉ MONDIALE & PROTOCOLE DE ROUTAGE REGIONAL
-- À exécuter sur l'instance ou le cluster Postgres Central / Global
-- =========================================================================

CREATE TABLE IF NOT EXISTS global_identity_registry (
    -- Clé primaire racine : garantit qu'un compte n'a qu'une seule ligne d'identité mondiale
    account_id   UUID PRIMARY KEY,
    
    -- Le pointeur de routage d'infrastructure (Ex: 'EU', 'US')
    region       VARCHAR(10) NOT NULL,
    
    -- Le sub_id de l'IdP (Keycloak/Google/Apple) mondialement unique et optionnel
    sub_id       TEXT NULL,
    
    -- Les hashs binaires SHA-256 (32 octets) pour l'anonymisation et l'indexation rapide
    email_hash   BYTEA NULL,
    phone_hash   BYTEA NULL,
    
    -- Alignement du cycle de vie (Ex: 'UNVERIFIED', 'PENDING', 'ACTIVE', 'SUSPENDED')
    state        TEXT NOT NULL DEFAULT 'UNVERIFIED',
    created_at   TIMESTAMPTZ NOT NULL,
    updated_at   TIMESTAMPTZ NOT NULL,

    --  CONTRAINTES D'UNICITÉ MONDIALE INDÉPENDANTES
    CONSTRAINT uq_global_email UNIQUE (email_hash),
    CONSTRAINT uq_global_phone UNIQUE (phone_hash),
    CONSTRAINT uq_global_sub_id UNIQUE (sub_id),

    -- REGLE METIER : Le compte doit posséder au moins un identifiant direct ou fédéré
    CONSTRAINT ck_global_has_identifier CHECK (
        email_hash IS NOT NULL OR phone_hash IS NOT NULL OR sub_id IS NOT NULL
    )
);

-- =========================================================================
-- INDEXATION & TRIGGERS
-- =========================================================================

-- Index de couverture pour accélérer le routage si on cherche un compte par son IdP (Login OAuth2)
CREATE INDEX IF NOT EXISTS idx_global_registry_sub_id 
ON global_identity_registry (sub_id) 
WHERE sub_id IS NOT NULL;

-- Trigger pour l'automatisation du updated_at (Réutilisation de ta fonction existante)
DROP TRIGGER IF EXISTS trg_set_timestamp_global_registry ON global_identity_registry;
CREATE TRIGGER trg_set_timestamp_global_registry 
    BEFORE UPDATE ON global_identity_registry 
    FOR EACH ROW 
    EXECUTE FUNCTION trigger_set_timestamp();