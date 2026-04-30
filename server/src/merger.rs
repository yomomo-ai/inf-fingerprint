//! Closed-loop identity correction.
//!
//! Two entry points:
//! 1. [`merge_visitors`] — explicit, caller-driven (e.g., user logs in and
//!    proves two visitor_ids belong to the same person).
//! 2. [`auto_merge_pass`] — background scan that finds high-confidence
//!    duplicate pairs via the recall buckets and merges them without
//!    operator intervention. Triggered on a tokio interval timer.
//!
//! Both end up calling [`merge_pair_in_tx`], which moves observations,
//! consolidates the visitors row, drops the merged signature, and appends
//! a row to `visitor_merges` so callers holding a stale visitor_id can
//! later resolve it via [`resolve_canonical`].

use anyhow::{Context, Result};
use sqlx::postgres::PgPool;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::bayes;
use crate::matcher::{BucketCache, SignatureRow};

/// Errors raised by [`merge_visitors`]. Distinguishes caller-input issues
/// (treat as 400) from internal failures (500). The previous behaviour
/// surfaced everything as 500 because a missing canonical hit an FK
/// violation deep inside the transaction — actionable on the operator
/// side but indistinguishable from a real bug for the caller.
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    /// First id in the input doesn't exist in `visitors` and isn't
    /// reachable through the merge audit chain. Almost always a stale
    /// browser cache or a fabricated id — return BadRequest to the
    /// caller so it can clear / re-identify and try again.
    #[error("unknown canonical visitor: {0}")]
    UnknownCanonical(Uuid),

    /// No visitor ids supplied at all. API-level validation should catch
    /// this before us, but the merger keeps its own guard for direct
    /// callers (e.g. [`auto_merge_pass`] passing arrays it built).
    #[error("merge_visitors: no visitor ids supplied")]
    EmptyInput,

    /// Anything else — DB connection drops, query plan errors, audit
    /// insert failures. Bubbled up as 500.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<sqlx::Error> for MergeError {
    fn from(err: sqlx::Error) -> Self {
        Self::Other(anyhow::Error::from(err))
    }
}

/// Score above which the auto-merge pass will collapse two visitors without
/// human review. Set well above `match_threshold` (12) and even above
/// `match_threshold + 10` (the "exact" cutoff) so the system only auto-acts
/// when the evidence is overwhelming.
const AUTO_MERGE_THRESHOLD: f64 = 30.0;

/// Maximum candidate pairs the auto-merge scan inspects per pass. Bounds
/// the work; remaining pairs are picked up on the next tick.
const AUTO_MERGE_BATCH: i64 = 64;

/// How often the background auto-merge task wakes up. The matcher already
/// merges most same-person duplicates synchronously via cookie fast-path
/// or recall-bucket scan; this is a janitor for the residual cases (cookie
/// cleared, near-simultaneous first visits across devices, etc.) and
/// doesn't need to be aggressive.
const AUTO_MERGE_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy)]
pub enum MergeSource {
    FeedbackApi,
    AutoMerge,
}

impl MergeSource {
    fn as_str(self) -> &'static str {
        match self {
            MergeSource::FeedbackApi => "feedback_api",
            MergeSource::AutoMerge => "auto_merge",
        }
    }
}

#[derive(Debug)]
pub struct MergeOutcome {
    pub canonical_visitor_id: Uuid,
    pub merged_visitor_ids: Vec<Uuid>,
    pub total_observation_count: i64,
}

/// Caller-driven merge. The first id in `visitor_ids` is the canonical;
/// every other id (de-duplicated, resolved through any prior merges) is
/// folded into it. Cache is invalidated wholesale — granular invalidation
/// would require knowing every recall key the merged visitors had been
/// indexed under, and the 60s TTL covers that case anyway.
///
/// Returns [`MergeError::UnknownCanonical`] when the first id resolves to
/// a visitor that doesn't exist (browser cached a stale id, never went
/// through `/v1/identify`, or hand-fabricated). The api handler maps that
/// to 400 — without the early check, the same input would reach
/// `merge_pair_in_tx`'s `UPDATE observations` and trip the FK constraint
/// on `observations.visitor_id`, surfacing as an opaque 500.
pub async fn merge_visitors(
    pool: &PgPool,
    cache: &BucketCache,
    visitor_ids: &[Uuid],
    reason: &str,
    source: MergeSource,
) -> Result<MergeOutcome, MergeError> {
    if visitor_ids.is_empty() {
        return Err(MergeError::EmptyInput);
    }
    let mut tx = pool.begin().await?;
    let canonical = resolve_canonical_in_tx(&mut tx, visitor_ids[0]).await?;

    // Guard: canonical must be a live visitor row. A stale id resolves to
    // itself (no audit hop) but won't be in `visitors` — letting it
    // through means the first `merge_pair_in_tx` call hits an FK violation
    // when it tries to reassign observations to a non-existent visitor.
    // Fail fast with a typed error so the caller gets BadRequest instead
    // of 500.
    let canonical_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM visitors WHERE visitor_id = $1)")
            .bind(canonical)
            .fetch_one(&mut *tx)
            .await
            .context("checking canonical visitor existence")?;
    if !canonical_exists {
        return Err(MergeError::UnknownCanonical(canonical));
    }

    let mut merged_ids = Vec::new();
    for &raw_id in &visitor_ids[1..] {
        let id = resolve_canonical_in_tx(&mut tx, raw_id).await?;
        if id == canonical || merged_ids.contains(&id) {
            continue;
        }
        merge_pair_in_tx(&mut tx, canonical, id, reason, source).await?;
        merged_ids.push(id);
    }
    let total: i64 =
        sqlx::query_scalar("SELECT observation_count FROM visitors WHERE visitor_id = $1")
            .bind(canonical)
            .fetch_one(&mut *tx)
            .await
            .context("reading post-merge observation_count")?;
    tx.commit().await?;

    // Wholesale cache flush: any cache entry might contain a merged-away
    // visitor's signature row (now deleted), which would surface a dead
    // visitor_id on the next score pass. Cheaper than tracking which
    // recall keys those rows were indexed under.
    cache.invalidate_all();

    Ok(MergeOutcome {
        canonical_visitor_id: canonical,
        merged_visitor_ids: merged_ids,
        total_observation_count: total,
    })
}

/// Follow the merge audit chain to find the current canonical id for
/// `visitor_id`. Used both internally (to normalize merge inputs) and
/// exposed via API so callers holding a stale id can refresh it.
pub async fn resolve_canonical(pool: &PgPool, visitor_id: Uuid) -> Result<Uuid> {
    let mut current = visitor_id;
    loop {
        let next: Option<Uuid> = sqlx::query_scalar(
            "SELECT canonical_visitor_id FROM visitor_merges WHERE merged_visitor_id = $1",
        )
        .bind(current)
        .fetch_optional(pool)
        .await
        .context("resolving merge chain")?;
        match next {
            Some(c) if c != current => current = c,
            _ => return Ok(current),
        }
    }
}

async fn resolve_canonical_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    visitor_id: Uuid,
) -> Result<Uuid> {
    let mut current = visitor_id;
    loop {
        let next: Option<Uuid> = sqlx::query_scalar(
            "SELECT canonical_visitor_id FROM visitor_merges WHERE merged_visitor_id = $1",
        )
        .bind(current)
        .fetch_optional(&mut **tx)
        .await
        .context("resolving merge chain in tx")?;
        match next {
            Some(c) if c != current => current = c,
            _ => return Ok(current),
        }
    }
}

async fn merge_pair_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    canonical: Uuid,
    merged: Uuid,
    reason: &str,
    source: MergeSource,
) -> Result<()> {
    // 1. Pull the merged visitor's observation count before we touch
    //    anything (audit record needs it).
    let merged_count: Option<i64> =
        sqlx::query_scalar("SELECT observation_count FROM visitors WHERE visitor_id = $1")
            .bind(merged)
            .fetch_optional(&mut **tx)
            .await
            .context("reading merged observation_count")?;
    let Some(merged_count) = merged_count else {
        // Already gone (concurrent merge). Skip silently — idempotent.
        return Ok(());
    };

    // 2. Reassign all observations to the canonical visitor.
    sqlx::query("UPDATE observations SET visitor_id = $1 WHERE visitor_id = $2")
        .bind(canonical)
        .bind(merged)
        .execute(&mut **tx)
        .await
        .context("reassigning observations")?;

    // 3. Bump the canonical's observation_count and last_seen.
    sqlx::query(
        r#"UPDATE visitors
              SET observation_count = observation_count + $2,
                  last_seen_at = GREATEST(last_seen_at,
                      (SELECT last_seen_at FROM visitors WHERE visitor_id = $3))
            WHERE visitor_id = $1"#,
    )
    .bind(canonical)
    .bind(merged_count)
    .bind(merged)
    .execute(&mut **tx)
    .await
    .context("updating canonical visitor")?;

    // 4. Drop the merged visitor. Cascade deletes signatures and
    //    signature_buckets via FKs.
    sqlx::query("DELETE FROM visitors WHERE visitor_id = $1")
        .bind(merged)
        .execute(&mut **tx)
        .await
        .context("deleting merged visitor")?;

    // 5. Audit record.
    sqlx::query(
        r#"INSERT INTO visitor_merges
              (canonical_visitor_id, merged_visitor_id, merged_observation_count, reason, source)
              VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(canonical)
    .bind(merged)
    .bind(merged_count)
    .bind(reason)
    .bind(source.as_str())
    .execute(&mut **tx)
    .await
    .context("inserting merge audit row")?;

    Ok(())
}

/// One iteration of the background auto-merge scan. Finds the most recent
/// observations that haven't yet been compared against bucket peers in
/// this pass, runs Bayes against any peer with overlapping recall keys,
/// and merges pairs that score above [`AUTO_MERGE_THRESHOLD`].
///
/// The query is intentionally simple — find pairs sharing any single
/// recall-bucket entry, score them, merge if strong. Performance is
/// bounded by [`AUTO_MERGE_BATCH`] per tick.
pub async fn auto_merge_pass(
    pool: &PgPool,
    cache: &BucketCache,
    match_threshold: f64,
) -> Result<u32> {
    // Find candidate pairs: two visitors sharing at least one recall
    // bucket entry. Take only pairs we haven't already merged (the
    // unique constraint on visitor_merges.merged_visitor_id naturally
    // filters out re-tries on the same id).
    let pairs: Vec<(Uuid, Uuid)> = sqlx::query_as(
        r#"
        WITH peer AS (
            SELECT DISTINCT
                LEAST(a.visitor_id, b.visitor_id)    AS lhs,
                GREATEST(a.visitor_id, b.visitor_id) AS rhs
            FROM signature_buckets a
            JOIN signature_buckets b
              ON a.bucket_kind = b.bucket_kind
             AND a.bucket_value = b.bucket_value
             AND a.visitor_id < b.visitor_id
        )
        SELECT lhs, rhs
        FROM peer
        WHERE NOT EXISTS (
            SELECT 1 FROM visitor_merges m
             WHERE (m.canonical_visitor_id = peer.lhs AND m.merged_visitor_id = peer.rhs)
                OR (m.canonical_visitor_id = peer.rhs AND m.merged_visitor_id = peer.lhs)
        )
        LIMIT $1
        "#,
    )
    .bind(AUTO_MERGE_BATCH)
    .fetch_all(pool)
    .await
    .context("fetching auto-merge candidate pairs")?;

    if pairs.is_empty() {
        return Ok(0);
    }

    let mut merged_count = 0u32;
    for (a, b) in pairs {
        // Each merge runs as its own transaction so a failure on one
        // pair doesn't roll back others. Re-fetch the live signatures —
        // a previous iteration in this batch may already have merged
        // away one of the ids.
        let Some((sig_a, sig_b)) = fetch_signature_pair(pool, a, b).await? else {
            continue;
        };

        let score_ab = bayes_score_signature_pair(&sig_a, &sig_b);
        let score_ba = bayes_score_signature_pair(&sig_b, &sig_a);
        // Take the symmetric average so the comparison doesn't depend on
        // which side is treated as "stored" vs "request".
        let score = (score_ab + score_ba) / 2.0;

        if score >= AUTO_MERGE_THRESHOLD {
            // Pick the older (more observations / earlier first_seen) as
            // canonical to preserve the longer history.
            let (canonical, merged) = if observation_count(pool, sig_a.visitor_id).await?
                >= observation_count(pool, sig_b.visitor_id).await?
            {
                (sig_a.visitor_id, sig_b.visitor_id)
            } else {
                (sig_b.visitor_id, sig_a.visitor_id)
            };

            let reason = format!(
                "bayes_score={:.2}, threshold={:.2}",
                score, AUTO_MERGE_THRESHOLD
            );
            if let Err(e) = merge_visitors(
                pool,
                cache,
                &[canonical, merged],
                &reason,
                MergeSource::AutoMerge,
            )
            .await
            {
                tracing::warn!(?e, %canonical, %merged, "auto-merge failed");
                continue;
            }
            merged_count += 1;
            tracing::info!(
                %canonical, %merged, score, threshold = AUTO_MERGE_THRESHOLD,
                "auto-merged duplicate visitors"
            );
        } else if score >= match_threshold {
            // Strong-but-not-overwhelming match: log so an operator can
            // inspect (e.g., manually feedback-merge if appropriate).
            tracing::debug!(
                lhs = %a, rhs = %b, score, threshold = AUTO_MERGE_THRESHOLD,
                "candidate pair below auto-merge threshold"
            );
        }
    }

    Ok(merged_count)
}

async fn fetch_signature_pair(
    pool: &PgPool,
    a: Uuid,
    b: Uuid,
) -> Result<Option<(SignatureRow, SignatureRow)>> {
    let rows: Vec<SignatureRow> = sqlx::query_as(const_format::concatcp!(
        "SELECT ",
        crate::matcher::SIG_COLUMNS,
        " FROM signatures WHERE visitor_id = ANY($1)"
    ))
    .bind(&[a, b][..])
    .fetch_all(pool)
    .await
    .context("loading signature pair")?;
    if rows.len() < 2 {
        return Ok(None);
    }
    let mut iter = rows.into_iter();
    let r0 = iter.next().unwrap();
    let r1 = iter.next().unwrap();
    Ok(Some((r0, r1)))
}

async fn observation_count(pool: &PgPool, visitor_id: Uuid) -> Result<i64> {
    let n: i64 = sqlx::query_scalar("SELECT observation_count FROM visitors WHERE visitor_id = $1")
        .bind(visitor_id)
        .fetch_one(pool)
        .await
        .context("reading observation_count")?;
    Ok(n)
}

/// Bayes-score one signature against another. We synthesize a `Features`
/// from the rhs signature and reuse the existing scorer rather than
/// duplicating the per-feature logic.
fn bayes_score_signature_pair(stored: &SignatureRow, candidate: &SignatureRow) -> f64 {
    let stored_sig = stored.to_signature();
    let candidate_features = candidate.synthesize_features();
    bayes::score(&stored_sig, &candidate_features).total
}

/// Spawn the background auto-merge task. Cheap idle cost: just a sleep
/// loop. Each tick logs how many pairs were merged.
pub fn spawn_auto_merge_task(pool: Arc<PgPool>, cache: BucketCache, match_threshold: f64) {
    tokio::spawn(async move {
        // First tick fires after the interval, not immediately, so the
        // server has time to settle on startup.
        let mut ticker = tokio::time::interval_at(
            tokio::time::Instant::now() + AUTO_MERGE_INTERVAL,
            AUTO_MERGE_INTERVAL,
        );
        loop {
            ticker.tick().await;
            match auto_merge_pass(&pool, &cache, match_threshold).await {
                Ok(0) => tracing::debug!("auto-merge pass: nothing to merge"),
                Ok(n) => tracing::info!(merged = n, "auto-merge pass complete"),
                Err(e) => tracing::warn!(?e, "auto-merge pass failed"),
            }
        }
    });
}
