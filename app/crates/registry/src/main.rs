//! BitModel registry — the VPS service that maps `model → live seeders`, serves
//! signed manifests, and accepts seeder announces/heartbeats.
//!
//! State is a single SQLite file (the only state in the whole system). Manifests
//! are stored as-is; clients verify the quorum signatures themselves, so the
//! registry is untrusted for *truth* — it only helps peers find each other.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Seeders not seen within this many seconds are considered offline.
const SEEDER_TTL_SECS: i64 = 600;

#[derive(Parser, Debug)]
#[command(name = "bitmodel-registry", about = "BitModel registry + manifest host")]
struct Args {
    /// Address to bind.
    #[arg(long, env = "BITMODEL_REGISTRY_BIND", default_value = "0.0.0.0:8090")]
    bind: String,
    /// SQLite database path.
    #[arg(long, env = "BITMODEL_REGISTRY_DB", default_value = "registry.db")]
    db: String,
    /// Bearer token required to publish manifests (PUT /manifest/:model).
    #[arg(long, env = "BITMODEL_REGISTRY_TOKEN", default_value = "")]
    token: String,
}

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Connection>>,
    token: String,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[derive(Deserialize)]
struct Announce {
    model: String,
    node_id: String,
}

#[derive(Serialize)]
struct Seeders {
    model: String,
    seeders: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();

    let conn = Connection::open(&args.db)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS seeders (
            model     TEXT NOT NULL,
            node_id   TEXT NOT NULL,
            last_seen INTEGER NOT NULL,
            PRIMARY KEY (model, node_id)
         );
         CREATE TABLE IF NOT EXISTS manifests (
            model    TEXT PRIMARY KEY,
            json     TEXT NOT NULL,
            updated  INTEGER NOT NULL
         );",
    )?;

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        token: args.token,
    };

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/announce", post(announce))
        .route("/seeders/{model}", get(seeders))
        .route("/manifest/{model}", get(get_manifest).put(put_manifest))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    tracing::info!("registry listening on {}", args.bind);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn announce(State(st): State<AppState>, Json(a): Json<Announce>) -> impl IntoResponse {
    let db = st.db.lock().unwrap();
    let res = db.execute(
        "INSERT INTO seeders (model, node_id, last_seen) VALUES (?1, ?2, ?3)
         ON CONFLICT(model, node_id) DO UPDATE SET last_seen = ?3",
        rusqlite::params![a.model, a.node_id, now()],
    );
    match res {
        Ok(_) => (StatusCode::OK, "ok").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn seeders(State(st): State<AppState>, Path(model): Path<String>) -> impl IntoResponse {
    let db = st.db.lock().unwrap();
    let cutoff = now() - SEEDER_TTL_SECS;
    // Opportunistically prune stale rows.
    let _ = db.execute(
        "DELETE FROM seeders WHERE last_seen < ?1",
        rusqlite::params![cutoff],
    );
    let mut stmt = match db.prepare(
        "SELECT node_id FROM seeders WHERE model = ?1 AND last_seen >= ?2 ORDER BY last_seen DESC",
    ) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let rows = stmt
        .query_map(rusqlite::params![model, cutoff], |r| r.get::<_, String>(0))
        .and_then(|it| it.collect::<Result<Vec<_>, _>>());
    match rows {
        Ok(seeders) => Json(Seeders { model, seeders }).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_manifest(State(st): State<AppState>, Path(model): Path<String>) -> impl IntoResponse {
    let db = st.db.lock().unwrap();
    let row: rusqlite::Result<String> = db.query_row(
        "SELECT json FROM manifests WHERE model = ?1",
        rusqlite::params![model],
        |r| r.get(0),
    );
    match row {
        Ok(json) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "no such model").into_response(),
    }
}

async fn put_manifest(
    State(st): State<AppState>,
    Path(model): Path<String>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    // Auth: require the bearer token if one is configured.
    if !st.token.is_empty() {
        let ok = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == format!("Bearer {}", st.token))
            .unwrap_or(false);
        if !ok {
            return (StatusCode::UNAUTHORIZED, "bad token").into_response();
        }
    }
    // Validate it parses as JSON (don't accept garbage).
    if serde_json::from_str::<serde_json::Value>(&body).is_err() {
        return (StatusCode::BAD_REQUEST, "body is not valid JSON").into_response();
    }
    let db = st.db.lock().unwrap();
    let res = db.execute(
        "INSERT INTO manifests (model, json, updated) VALUES (?1, ?2, ?3)
         ON CONFLICT(model) DO UPDATE SET json = ?2, updated = ?3",
        rusqlite::params![model, body, now()],
    );
    match res {
        Ok(_) => (StatusCode::OK, "published").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
