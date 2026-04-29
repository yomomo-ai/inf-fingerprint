-- Multi-dimensional recall index for the matcher.
--
-- Replaces the single composite bucket_key (which acted as a hard filter,
-- contradicting the Bayesian premise: any single feature could veto a
-- candidate from being scored at all). Each signature is now indexed under
-- multiple stable, high-cardinality features as separate "recall hooks".
-- On identify, the request computes its recall keys for each dimension and
-- ANY signature matching ANY dimension enters the candidate pool. Bayes
-- scores the pool with the full feature set + calibrated weights —
-- bucketing is purely a performance optimization, not a discriminator.
--
-- bucket_kind enum (must match constants in server/src/features.rs):
--   0 = canvas_hash
--   1 = webgl_render_hash
--   2 = webgl_params_hash
--   3 = fonts_sorted_hash
--   4 = audio_hash

CREATE TABLE IF NOT EXISTS signature_buckets (
    visitor_id   UUID     NOT NULL REFERENCES signatures(visitor_id) ON DELETE CASCADE,
    bucket_kind  SMALLINT NOT NULL,
    bucket_value TEXT     NOT NULL,
    PRIMARY KEY (bucket_kind, bucket_value, visitor_id)
);

CREATE INDEX IF NOT EXISTS idx_sigbucket_visitor
    ON signature_buckets (visitor_id);

ALTER TABLE signatures   DROP COLUMN bucket_key;
ALTER TABLE observations DROP COLUMN bucket_key;
