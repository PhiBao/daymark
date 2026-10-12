//! HTTP API.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::artifacts::ArtifactStore;
use crate::brief::{self, BriefResponse, SERVICE_LEVEL};
use crate::config::Config;
use crate::error::{ApiResult, AppError};
use crate::ledger::{self, NewDecision};
use crate::llm::LlmClient;
use crate::sim::{self, PolicyKind, SimOutcome};

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub artifacts: Arc<ArtifactStore>,
    pub llm: Arc<LlmClient>,
    pub db: SqlitePool,
    pub sim_cache: Arc<Mutex<HashMap<String, SimOutcome>>>,
}

fn parse_date(s: &str) -> ApiResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| AppError::BadRequest(format!("bad date {s}, expected YYYY-MM-DD")))
}

async fn health(State(st): State<AppState>) -> Json<Value> {
    let reachable = tokio::time::timeout(std::time::Duration::from_secs(3), st.llm.models()).await;
    let (llm_ok, model_seen) = match reachable {
        Ok(Ok(v)) => {
            let seen = v
                .get("data")
                .and_then(|d| d.as_array())
                .and_then(|a| a.first())
                .and_then(|m| m.get("id"))
                .and_then(|i| i.as_str())
                .map(|s| s.to_string());
            (true, seen)
        }
        _ => (false, None),
    };
    Json(json!({
        "ok": true,
        "service": "daymark-api",
        "artifacts": {
            "forecast_window": [st.artifacts.forecast_start.to_string(), st.artifacts.forecast_end.to_string()],
            "stores": st.artifacts.stores.keys().collect::<Vec<_>>(),
            "items": st.artifacts.items.len(),
        },
        "llm": {
            "base_url": st.llm.base_url(),
            "model": st.llm.model(),
            "reachable": llm_ok,
            "reported_model": model_seen,
            "cost_model": st.llm.cost,
        }
    }))
}

async fn meta(State(st): State<AppState>) -> Json<Value> {
    Json(json!({
        "meta": st.artifacts.meta,
        "eval_metrics": st.artifacts.eval_metrics,
        "promo_lift": st.artifacts.promo_lift,
        "benchmark": st.artifacts.benchmark,
        "cost_model": st.llm.cost,
        "llm_model": st.llm.model(),
        "stores": st.artifacts.stores.values().collect::<Vec<_>>(),
        "families": st.artifacts.families,
        "service_level": SERVICE_LEVEL,
    }))
}

#[derive(Deserialize)]
struct BriefQuery {
    date: Option<String>,
    refresh: Option<bool>,
}

async fn brief_facts(State(st): State<AppState>, Path(store): Path<i16>, Query(q): Query<BriefQuery>) -> ApiResult<Json<Value>> {
    let date = parse_date(q.date.as_deref().unwrap_or("2017-08-14"))?;
    let art = st.artifacts.clone();
    let facts = tokio::task::spawn_blocking(move || brief::build_facts(&art, store, date))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;
    Ok(Json(json!({ "facts": facts })))
}

async fn brief_generate(
    State(st): State<AppState>,
    Path(store): Path<i16>,
    Query(q): Query<BriefQuery>,
) -> ApiResult<Json<BriefResponse>> {
    let date = parse_date(q.date.as_deref().unwrap_or("2017-08-14"))?;
    let refresh = q.refresh.unwrap_or(false);
    let date_s = date.to_string();

    if !refresh {
        if let Some(cached) = ledger::get_cached_brief(&st.db, store, &date_s).await? {
            if let Ok(mut resp) = serde_json::from_value::<BriefResponse>(cached.payload.clone()) {
                resp.cached = true;
                resp.generated_at = cached.created_at;
                return Ok(Json(resp));
            }
        }
    }

    let art = st.artifacts.clone();
    let facts = tokio::task::spawn_blocking(move || brief::build_facts(&art, store, date))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;

    let outcome = brief::generate_narrative(&st.llm, &facts)
        .await
        .map_err(|e| AppError::Upstream(e.to_string()))?;

    let cost = st.llm.usd(&outcome.usage);
    let resp = BriefResponse {
        facts,
        narrative: Some(outcome.narrative),
        grounding: Some(outcome.grounding),
        usage: Some(outcome.usage),
        cost_usd: Some(cost),
        model: st.llm.model().to_string(),
        source: outcome.source,
        generated_at: chrono::Utc::now().to_rfc3339(),
        cached: false,
    };

    let payload = serde_json::to_value(&resp).unwrap_or(json!({}));
    ledger::cache_brief(&st.db, store, &date_s, &payload, resp.grounding.as_ref().map(|g| g.ok).unwrap_or(false), &json!(resp.usage))
        .await
        .map_err(|e| AppError::Internal(e))?;

    Ok(Json(resp))
}

async fn brief_published(
    State(st): State<AppState>,
    Path(store): Path<i16>,
    Query(q): Query<BriefQuery>,
) -> ApiResult<Json<BriefResponse>> {
    let date = parse_date(q.date.as_deref().unwrap_or("2017-08-14"))?;
    let date_s = date.to_string();
    let cached = ledger::get_cached_brief(&st.db, store, &date_s)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no published brief for store {store} on {date_s}")))?;
    let mut resp: BriefResponse = serde_json::from_value(cached.payload)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("bad cached brief: {e}")))?;
    resp.cached = true;
    resp.generated_at = cached.created_at;
    Ok(Json(resp))
}

async fn decide(State(st): State<AppState>, Json(d): Json<NewDecision>) -> ApiResult<Json<Value>> {
    if !["approve", "edit", "flag", "reject"].contains(&d.action.as_str()) {
        return Err(AppError::BadRequest("action must be approve|edit|flag|reject".into()));
    }
    let entry = ledger::append(&st.db, d).await?;
    Ok(Json(json!({ "entry": entry })))
}

#[derive(Deserialize)]
struct LedgerQuery {
    store: Option<i16>,
}

async fn ledger_list(State(st): State<AppState>, Query(q): Query<LedgerQuery>) -> ApiResult<Json<Value>> {
    let (entries, chain_ok) = ledger::list(&st.db, q.store).await?;
    Ok(Json(json!({ "entries": entries, "chain_ok": chain_ok, "count": entries.len() })))
}

#[derive(Deserialize)]
struct ReplayQuery {
    store: Option<i16>,
    start: Option<String>,
    end: Option<String>,
    service_level: Option<f64>,
    lift_scale: Option<f64>,
    shelf_life_scale: Option<f64>,
}

async fn replay(State(st): State<AppState>, Query(q): Query<ReplayQuery>) -> ApiResult<Json<Value>> {
    let store = q.store.unwrap_or(2);
    let default_start = st.artifacts.forecast_start + chrono::Duration::days(4);
    let default_end = st.artifacts.forecast_end - chrono::Duration::days(2);
    let start = q.start.as_deref().map(parse_date).transpose()?.unwrap_or(default_start);
    let end = q.end.as_deref().map(parse_date).transpose()?.unwrap_or(default_end);
    if end <= start {
        return Err(AppError::BadRequest("end must be after start".into()));
    }
    let service_level = q.service_level.unwrap_or(SERVICE_LEVEL).clamp(0.5, 0.99);
    let lift_scale = q.lift_scale.unwrap_or(1.0).clamp(0.0, 3.0);
    let shelf_life_scale = q.shelf_life_scale.unwrap_or(1.0).clamp(0.3, 2.0);

    let key = format!("{store}|{start}|{end}|{service_level:.3}|{lift_scale:.3}|{shelf_life_scale:.3}");
    let cached = st.sim_cache.lock().unwrap().get(&key).cloned();
    let daymark_outcome = match cached {
        Some(o) => o,
        None => {
            let art = st.artifacts.clone();
            let input = {
                let items = brief::sim_items(&art, store);
                sim::build_input(store, items, start, end, service_level, lift_scale, shelf_life_scale)
            };
            let out = tokio::task::spawn_blocking(move || sim::run(&input, PolicyKind::Daymark))
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
            st.sim_cache.lock().unwrap().insert(key, out.clone());
            out
        }
    };
    let (spreadsheet, forecast_only) = {
        let art = st.artifacts.clone();
        let items_a = brief::sim_items(&art, store);
        let items_b = items_a.clone();
        let (a, b) = tokio::task::spawn_blocking(move || {
            let ia = sim::build_input(store, items_a, start, end, service_level, lift_scale, shelf_life_scale);
            let ib = sim::build_input(store, items_b, start, end, service_level, lift_scale, shelf_life_scale);
            (
                sim::run(&ia, PolicyKind::Spreadsheet),
                sim::run(&ib, PolicyKind::ForecastOnly),
            )
        })
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        (a, b)
    };

    let d = |a: &SimOutcome, b: &SimOutcome| {
        json!({
            "waste_units_delta": b.totals.waste_units - a.totals.waste_units,
            "waste_units_delta_pct": if a.totals.waste_units > 0.0 { (b.totals.waste_units - a.totals.waste_units) / a.totals.waste_units * 100.0 } else { 0.0 },
            "lost_units_delta": b.totals.lost_units - a.totals.lost_units,
            "lost_units_delta_pct": if a.totals.lost_units > 0.0 { (b.totals.lost_units - a.totals.lost_units) / a.totals.lost_units * 100.0 } else { 0.0 },
            "waste_value_delta_usd": b.totals.waste_value_usd - a.totals.waste_value_usd,
            "service_level_delta_pts": b.totals.service_level_pct - a.totals.service_level_pct,
            "sales_units_delta": b.totals.sales_units - a.totals.sales_units,
        })
    };

    Ok(Json(json!({
        "store": store,
        "window": {"start": start, "end": end, "days": (end - start).num_days() + 1},
        "params": {"service_level": service_level, "lift_scale": lift_scale, "shelf_life_scale": shelf_life_scale},
        "policies": [spreadsheet, forecast_only, daymark_outcome.clone()],
        "deltas": {
            "daymark_vs_spreadsheet": d(&spreadsheet, &daymark_outcome),
            "daymark_vs_forecast_only": d(&forecast_only, &daymark_outcome),
        },
        "note": "Deterministic simulator; identical demand stream and opening inventory for every policy. Demand = recorded sales (stockout censoring acknowledged as a limitation)."
    })))
}

async fn trust(State(st): State<AppState>) -> Json<Value> {
    Json(json!({
        "eval_metrics": st.artifacts.eval_metrics,
        "promo_lift": st.artifacts.promo_lift,
        "benchmark": st.artifacts.benchmark,
        "assumptions": st.artifacts.meta.get("assumptions"),
        "source": st.artifacts.meta.get("source"),
        "cost_model": st.llm.cost,
        "model": st.llm.model(),
        "grounding": "Every narrative figure is verified against the facts payload; failures are regenerated once, then replaced by a deterministic template (source field shows which path ran).",
    }))
}

async fn benchmark(State(st): State<AppState>) -> Json<Value> {
    Json(json!({ "benchmark": st.artifacts.benchmark }))
}

pub fn router(st: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/meta", get(meta))
        .route("/api/brief/{store}", get(brief_facts))
        .route("/api/brief/{store}/published", get(brief_published))
        .route("/api/brief/{store}/generate", post(brief_generate))
        .route("/api/decisions", post(decide))
        .route("/api/ledger", get(ledger_list))
        .route("/api/replay", get(replay))
        .route("/api/trust", get(trust))
        .route("/api/benchmark", get(benchmark))
        .with_state(st)
}
