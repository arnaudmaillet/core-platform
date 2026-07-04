-- Author tier, denormalized from social-graph.author_tier_changed (0=Standard,
-- 1=Premium, 2=Vip). Additive: existing rows read NULL → treated as Standard.
ALTER TABLE profile.profiles ADD tier tinyint;
