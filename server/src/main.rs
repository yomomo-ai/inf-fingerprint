use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;

mod api;
mod bayes;
mod config;
mod error;
mod features;
mod matcher;
mod merger;
mod store;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cfg = config::Config::load()?;
    let pool = store::connect(&cfg).await?;
    store::migrate(&pool, &cfg.database.schema).await?;

    let bucket_cache = matcher::new_bucket_cache(
        cfg.matcher.bucket_cache.capacity,
        cfg.matcher.bucket_cache.ttl.as_secs(),
    );

    let state = api::AppState {
        pool: pool.clone(),
        bucket_cache: bucket_cache.clone(),
        match_threshold: cfg.matcher.match_threshold,
        ambiguous_threshold: cfg.matcher.ambiguous_threshold,
        max_candidates: cfg.matcher.max_candidates,
    };

    // Background janitor: every few minutes, find pairs of visitors that
    // share recall-bucket entries but are stored separately, score them,
    // and auto-merge any that the matcher would have caught had they
    // arrived in the same bucket scan. Catches the residual cases where
    // synchronous matching couldn't (cookie cleared, near-simultaneous
    // first visits across devices).
    merger::spawn_auto_merge_task(Arc::new(pool), bucket_cache, cfg.matcher.match_threshold);

    let app = api::router(state);

    let listener = tokio::net::TcpListener::bind(&cfg.server.bind).await?;
    tracing::info!(bind = %cfg.server.bind, "listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("inf_fingerprint_server=debug,tower_http=info"));
    fmt().with_env_filter(env_filter).with_target(false).init();
}
