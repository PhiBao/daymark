//! Append-only decision ledger with a hash chain (tamper-evident audit trail).
//!
//! Every approved/edited/flagged decision is hashed together with the previous
//! entry's hash, so any later modification of history is detectable via
//! `GET /api/ledger` (entries are verified end-to-end on read).

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

const GENESIS: &str = "daymark-genesis";

#[derive(Clone, Debug, Deserialize)]
pub struct NewDecision {
    pub store: i16,
    pub brief_date: String,
    pub item: Option<i32>,
    pub action: String, // approve | edit | flag | reject
    pub qty: Option<f64>,
    pub reason: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LedgerEntry {
    pub id: i64,
    pub ts: String,
    pub store: i16,
    pub brief_date: String,
    pub item: Option<i32>,
    pub action: String,
    pub qty: Option<f64>,
    pub reason: Option<serde_json::Value>,
    pub prev_hash: String,
    pub hash: String,
    pub chain_ok: bool,
}

fn entry_hash(prev: &str, ts: &str, d: &NewDecision) -> String {
    let mut h = Sha256::new();
    let payload = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        prev,
        ts,
        d.store,
        d.brief_date,
        d.item.map(|i| i.to_string()).unwrap_or_default(),
        d.action,
        d.qty.map(|q| format!("{q:.3}")).unwrap_or_default(),
        d.reason.as_ref().map(|r| r.to_string()).unwrap_or_default(),
    );
    h.update(payload.as_bytes());
    let out = h.finalize();
    hex::encode(out)
}

pub async fn init(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts TEXT NOT NULL,
            store INTEGER NOT NULL,
            brief_date TEXT NOT NULL,
            item INTEGER,
            action TEXT NOT NULL,
            qty REAL,
            reason TEXT,
            prev_hash TEXT NOT NULL,
            hash TEXT NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS briefs (
            store INTEGER NOT NULL,
            brief_date TEXT NOT NULL,
            payload TEXT NOT NULL,
            grounding_ok INTEGER NOT NULL,
            usage TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (store, brief_date)
        )"#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn append(pool: &SqlitePool, d: NewDecision) -> Result<LedgerEntry> {
    let prev: Option<String> =
        sqlx::query("SELECT hash FROM decisions ORDER BY id DESC LIMIT 1")
            .fetch_optional(pool)
            .await?
            .map(|r| r.get::<String, _>("hash"));
    let prev_hash = prev.unwrap_or_else(|| GENESIS.to_string());
    let ts = Utc::now().to_rfc3339();
    let hash = entry_hash(&prev_hash, &ts, &d);
    let res = sqlx::query(
        r#"INSERT INTO decisions (ts, store, brief_date, item, action, qty, reason, prev_hash, hash)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
    )
    .bind(&ts)
    .bind(d.store)
    .bind(&d.brief_date)
    .bind(d.item)
    .bind(&d.action)
    .bind(d.qty)
    .bind(d.reason.as_ref().map(|r| r.to_string()))
    .bind(&prev_hash)
    .bind(&hash)
    .execute(pool)
    .await?;
    Ok(LedgerEntry {
        id: res.last_insert_rowid(),
        ts,
        store: d.store,
        brief_date: d.brief_date,
        item: d.item,
        action: d.action,
        qty: d.qty,
        reason: d.reason,
        prev_hash,
        hash,
        chain_ok: true,
    })
}

/// Read entries and re-verify the whole hash chain.
pub async fn list(pool: &SqlitePool, store: Option<i16>) -> Result<(Vec<LedgerEntry>, bool)> {
    let rows = match store {
        Some(s) => sqlx::query("SELECT * FROM decisions WHERE store = ?1 ORDER BY id ASC")
            .bind(s)
            .fetch_all(pool)
            .await?,
        None => sqlx::query("SELECT * FROM decisions ORDER BY id ASC").fetch_all(pool).await?,
    };
    let mut entries = Vec::new();
    let mut chain_ok = true;
    let mut prev_hash = GENESIS.to_string();
    for r in rows {
        let ts: String = r.get("ts");
        let store: i16 = r.get("store");
        let brief_date: String = r.get("brief_date");
        let item: Option<i32> = r.get("item");
        let action: String = r.get("action");
        let qty: Option<f64> = r.get("qty");
        let reason: Option<String> = r.get("reason");
        let stored_prev: String = r.get("prev_hash");
        let stored_hash: String = r.get("hash");
        let reason_json = reason.and_then(|s| serde_json::from_str(&s).ok());
        let recomputed = entry_hash(
            &stored_prev,
            &ts,
            &NewDecision { store, brief_date: brief_date.clone(), item, action: action.clone(), qty, reason: reason_json.clone() },
        );
        let ok = stored_hash == recomputed && stored_prev == prev_hash;
        chain_ok = chain_ok && ok;
        prev_hash = stored_hash.clone();
        entries.push(LedgerEntry {
            id: r.get("id"),
            ts,
            store,
            brief_date,
            item,
            action,
            qty,
            reason: reason_json,
            prev_hash: stored_prev,
            hash: stored_hash,
            chain_ok: ok,
        });
    }
    Ok((entries, chain_ok))
}

pub struct CachedBrief {
    pub payload: serde_json::Value,
    pub grounding_ok: bool,
    pub usage: serde_json::Value,
    pub created_at: String,
}

pub async fn cache_brief(
    pool: &SqlitePool,
    store: i16,
    brief_date: &str,
    payload: &serde_json::Value,
    grounding_ok: bool,
    usage: &serde_json::Value,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO briefs (store, brief_date, payload, grounding_ok, usage, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)
           ON CONFLICT(store, brief_date) DO UPDATE SET
             payload = excluded.payload, grounding_ok = excluded.grounding_ok,
             usage = excluded.usage, created_at = excluded.created_at"#,
    )
    .bind(store)
    .bind(brief_date)
    .bind(payload.to_string())
    .bind(grounding_ok as i64)
    .bind(usage.to_string())
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_cached_brief(pool: &SqlitePool, store: i16, brief_date: &str) -> Result<Option<CachedBrief>> {
    let row = sqlx::query("SELECT * FROM briefs WHERE store = ?1 AND brief_date = ?2")
        .bind(store)
        .bind(brief_date)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| CachedBrief {
        payload: serde_json::from_str(&r.get::<String, _>("payload")).unwrap_or(serde_json::json!({})),
        grounding_ok: r.get::<i64, _>("grounding_ok") != 0,
        usage: serde_json::from_str(&r.get::<String, _>("usage")).unwrap_or(serde_json::json!({})),
        created_at: r.get("created_at"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> SqlitePool {
        let p = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init(&p).await.unwrap();
        p
    }

    #[tokio::test]
    async fn chain_verifies_and_detects_tampering() {
        let p = pool().await;
        for i in 0..3 {
            append(&p, NewDecision {
                store: 2,
                brief_date: "2017-08-14".into(),
                item: Some(100 + i),
                action: "approve".into(),
                qty: Some(5.0),
                reason: Some(serde_json::json!({"why": "test"})),
            })
            .await
            .unwrap();
        }
        let (entries, ok) = list(&p, None).await.unwrap();
        assert_eq!(entries.len(), 3);
        assert!(ok, "fresh chain must verify");

        // tamper with the middle entry directly
        sqlx::query("UPDATE decisions SET qty = 999 WHERE id = 2")
            .execute(&p)
            .await
            .unwrap();
        let (entries, ok) = list(&p, None).await.unwrap();
        assert!(!ok, "tampering must be detected");
        assert!(entries.iter().any(|e| !e.chain_ok));
    }

    #[tokio::test]
    async fn brief_cache_roundtrip() {
        let p = pool().await;
        let payload = serde_json::json!({"headline": "test"});
        cache_brief(&p, 2, "2017-08-14", &payload, true, &serde_json::json!({"total_tokens": 10}))
            .await
            .unwrap();
        let got = get_cached_brief(&p, 2, "2017-08-14").await.unwrap().unwrap();
        assert!(got.grounding_ok);
        assert_eq!(got.payload["headline"], "test");
    }
}
