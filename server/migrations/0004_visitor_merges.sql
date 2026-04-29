-- Audit log for closed-loop identity correction.
--
-- The matcher is open-loop by default: every identify call commits to a
-- visitor_id without later opportunity to revise. In real deployments
-- callers eventually learn ground-truth identity (user logs in, binds a
-- phone number, completes KYC) and can tell us "these N visitor_ids were
-- actually one person" via the /v1/feedback endpoint.
--
-- A merge rewrites observations from the merged_visitor_ids onto the
-- canonical_visitor_id, combines observation_count, deletes the merged
-- visitors (cascades delete signatures + signature_buckets), and appends
-- one row here per merged-from id. The audit log is append-only and
-- never deleted — it lets future identify calls resolve a stale
-- visitor_id (returned to a caller before the merge) back to its
-- current canonical id.

CREATE TABLE IF NOT EXISTS visitor_merges (
    merge_id                 BIGSERIAL    PRIMARY KEY,
    canonical_visitor_id     UUID         NOT NULL,
    merged_visitor_id        UUID         NOT NULL UNIQUE,
    merged_at                TIMESTAMPTZ  NOT NULL DEFAULT now(),
    merged_observation_count BIGINT       NOT NULL,
    reason                   TEXT         NOT NULL,
    source                   TEXT         NOT NULL  -- 'feedback_api' | 'auto_merge'
);

-- Look up "what did this visitor id become" — supports caller resolving
-- a previously-returned visitor_id that has since been merged away.
CREATE INDEX IF NOT EXISTS idx_merges_merged
    ON visitor_merges (merged_visitor_id);

-- Reverse direction: list everything that was merged INTO a given canonical.
CREATE INDEX IF NOT EXISTS idx_merges_canonical
    ON visitor_merges (canonical_visitor_id);

-- High-confidence auto-merge threshold lives in code (matcher::AUTO_MERGE_THRESHOLD).
-- The background scanner finds candidate pairs via signature_buckets
-- (already-indexed recall hooks) and runs Bayes. Pairs scoring well above
-- match_threshold + safety margin are auto-merged with source='auto_merge'.
