-- =========================================================================
-- TABLE DE PROJECTION LOCALE : Réplication asynchrone des Profils (CQRS)
-- Optimisée pour : LOOKUP direct par profile_id lors de l'hydratation des posts
-- =========================================================================

CREATE TABLE IF NOT EXISTS projection_profiles_v1 (
    profile_id uuid,
    handle text,
    display_name text,
    avatar_url text,       -- Stocké sous forme de texte (NULL si non renseigné)
    is_verified boolean,
    updated_at timestamp,  -- Crucial pour la protection contre le désordre des messages Kafka
    PRIMARY KEY (profile_id)
) WITH comment = 'Copie locale dénormalisée des profils pour l''hydratation sub-milliseconde du fil d''actualité';

-- Index secondaire pour permettre la résolution inverse (Slug/Handle -> ID) au besoin en lecture locale
CREATE INDEX IF NOT EXISTS ON projection_profiles_v1 (handle);