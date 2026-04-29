use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database: Database,
    pub server: Server,
    pub matcher: Matcher,
}

#[derive(Debug, Deserialize)]
pub struct Database {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub db_name: String,
    pub schema: String,
    pub pool: Pool,
}

#[derive(Debug, Deserialize)]
pub struct Pool {
    pub max_conns: u32,
    pub min_conns: u32,
    #[serde(with = "humantime_serde")]
    pub max_conn_lifetime: Duration,
    #[serde(with = "humantime_serde")]
    pub max_conn_idle_time: Duration,
    #[serde(with = "humantime_serde")]
    #[allow(dead_code)] // sqlx pools merge connect/acquire timeouts under acquire_timeout
    pub connect_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub acquire_timeout: Duration,
    #[serde(default, with = "humantime_serde")]
    #[allow(dead_code)] // sqlx tests on acquire — informational
    pub health_check_period: Duration,
    #[serde(with = "humantime_serde")]
    pub statement_timeout: Duration,
}

#[derive(Debug, Deserialize)]
pub struct Server {
    pub bind: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Matcher {
    pub match_threshold: f64,
    pub ambiguous_threshold: f64,
    pub max_candidates: usize,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = std::env::var("INF_FP_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("config.toml"));
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).with_context(|| {
            format!(
                "reading config from {} (set INF_FP_CONFIG to override; copy config.toml.example as a template)",
                path.display()
            )
        })?;
        let cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("parsing TOML config at {}", path.display()))?;
        Ok(cfg)
    }

    pub fn database_url(&self) -> String {
        let db = &self.database;
        let user = urlencode(&db.user);
        let pass = urlencode(&db.password);
        format!(
            "postgres://{}:{}@{}:{}/{}",
            user, pass, db.host, db.port, db.db_name
        )
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
