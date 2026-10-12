mod api;
mod artifacts;
mod brief;
mod config;
mod cost;
mod error;
mod grounding;
mod ledger;
mod llm;
mod policy;
mod sim;

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::api::AppState;
use crate::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "daymark_api=info,tower_http=warn".into()),
        )
        .init();

    let cfg = Config::from_env();
    tracing::info!(?cfg, "starting daymark-api");

    let artifacts = artifacts::ArtifactStore::load(&cfg.data_dir).with_context(|| {
        format!(
            "loading artifacts from {} — run the engine pipeline first: uv run python -m daymark.train && uv run python -m daymark.export_serving",
            cfg.data_dir.display()
        )
    })?;
    tracing::info!(
        items = artifacts.items.len(),
        stores = ?artifacts.stores.keys().collect::<Vec<_>>(),
        window = %format!("{}..{}", artifacts.forecast_start, artifacts.forecast_end),
        "artifacts loaded"
    );

    std::fs::create_dir_all(&cfg.state_dir).context("creating state dir")?;
    let db_path = cfg.state_dir.join("daymark.db");
    let db = sqlx::SqlitePool::connect(&format!("sqlite://{}?mode=rwc", db_path.display())).await?;
    ledger::init(&db).await?;

    let llm = llm::LlmClient::new(&cfg, artifacts.benchmark.as_ref());
    tracing::info!(
        base = llm.base_url(),
        model = llm.model(),
        usd_per_1m_tokens = llm.cost.usd_per_1m_tokens,
        source = llm.cost.source,
        "model client ready"
    );

    let state = AppState {
        cfg: Arc::new(cfg.clone()),
        artifacts: Arc::new(artifacts),
        llm: Arc::new(llm),
        db,
        sim_cache: Arc::new(Mutex::new(Default::default())),
    };

    let app = Router::new()
        .merge(api::router(state))
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{}", cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
