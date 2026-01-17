-- ==========================================
-- GAMIFICATION & ENGAGEMENT
-- ==========================================

CREATE TABLE IF NOT EXISTS user_stats (
                                          account_id UUID NOT NULL,
                                          region_code VARCHAR(10) NOT NULL,

    -- Progression
    current_xp BIGINT NOT NULL DEFAULT 0,
    current_level INT NOT NULL DEFAULT 1,
    total_points BIGINT NOT NULL DEFAULT 0,

    -- Rétention (Streaks)
    current_streak INT NOT NULL DEFAULT 0,
    best_streak INT NOT NULL DEFAULT 0,
    last_activity_date DATE,

    -- Technique (OCC & Audit)
    version INT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (account_id, region_code)
    );

-- Trigger pour l'auto-update du timestamp
-- (Note : La fonction trigger_set_timestamp doit avoir été créée par shared-kernel)
CREATE TRIGGER trg_set_timestamp_stats
    BEFORE UPDATE ON user_stats
    FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

-- ==========================================
-- INDEXES DE PERFORMANCE HYPERSCALE
-- ==========================================

-- 1. Index pour les Leaderboards (Classement mondial ou régional)
-- L'indexation DESC sur XP permet de récupérer le TOP 100 instantanément.
-- On inclut account_id pour le "keyset pagination" (stable sorting).
CREATE INDEX IF NOT EXISTS idx_user_stats_leaderboard
    ON user_stats (current_xp DESC, account_id DESC);

-- 2. Index pour le calcul des niveaux (optionnel selon ton algorithme)
CREATE INDEX IF NOT EXISTS idx_user_stats_level
    ON user_stats (current_level DESC);