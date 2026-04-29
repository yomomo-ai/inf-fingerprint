use anyhow::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Executor;

use crate::config::Config;

pub async fn connect(cfg: &Config) -> Result<PgPool> {
    let url = cfg.database_url();
    let schema = cfg.database.schema.clone();
    let stmt_timeout_ms = cfg.database.pool.statement_timeout.as_millis() as i64;

    PgPoolOptions::new()
        .max_connections(cfg.database.pool.max_conns)
        .min_connections(cfg.database.pool.min_conns)
        .max_lifetime(cfg.database.pool.max_conn_lifetime)
        .idle_timeout(cfg.database.pool.max_conn_idle_time)
        .acquire_timeout(cfg.database.pool.acquire_timeout)
        .test_before_acquire(true)
        .after_connect(move |conn, _| {
            let schema = schema.clone();
            Box::pin(async move {
                conn.execute(format!("SET search_path TO {}, public", schema).as_str())
                    .await?;
                conn.execute(format!("SET statement_timeout = {}", stmt_timeout_ms).as_str())
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .context("connecting to postgres")
}

pub async fn migrate(pool: &PgPool, schema: &str) -> Result<()> {
    // Ensure the schema exists before running migrations; sqlx migrator will
    // use the search_path from after_connect for everything else.
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {}", schema))
        .execute(pool)
        .await
        .with_context(|| format!("creating schema {}", schema))?;

    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("running migrations")
}
