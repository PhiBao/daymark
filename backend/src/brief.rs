//! Brief assembly: facts first, language second.
//!
//! Every number in a Daymark brief is computed here from the forecaster and the
//! simulator; the LLM only turns those facts into an editor's front page, and
//! its copy is verified by the grounding checker before it is ever shown.

use std::collections::BTreeMap;

use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::artifacts::{ArtifactStore, CalRow, Store};
use crate::grounding::{collect_facts, verify, GroundingReport};
use crate::llm::{extract_json, LlmClient, LlmUsage};
use crate::policy::{markdown_cut, markdown_demand_boost, order_advice};
use crate::sim::{self, PolicyKind, SimItem};

pub const SERVICE_LEVEL: f64 = 0.92;
/// Promo lift measured on Favorita produce medians above 140%; the *policy* uses
/// a conservative cap so planning is not driven by the most aggressive promos.
/// The raw measurement is always shown alongside.
pub const MARKDOWN_LIFT_CAP: f64 = 40.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhyChip {
    pub feature: String,
    pub label: String,
    pub contrib: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderLine {
    pub item: i32,
    pub label: String,
    pub family: String,
    pub onhand: f64,
    pub cover_days: f64,
    pub p10: f64,
    pub p50: f64,
    pub p90: f64,
    pub suggest_qty: i32,
    pub unit_price_usd: f64,
    pub shelf_life_days: i16,
    pub promo_today: bool,
    pub why: Vec<WhyChip>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarkdownLine {
    pub item: i32,
    pub label: String,
    pub family: String,
    pub onhand: f64,
    pub expires_in_days: i64,
    pub cut_pct: f64,
    pub expected_clear_units: f64,
    pub recovered_usd: f64,
    pub waste_avoided_usd: f64,
    pub why: Vec<WhyChip>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromoLine {
    pub item: i32,
    pub label: String,
    pub family: String,
    pub lift_pct: f64,
    pub lift_measured_pct: f64,
    pub cover_days: f64,
    pub onhand: f64,
    pub expected_incremental_units: f64,
    pub why: Vec<WhyChip>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchLine {
    pub item: i32,
    pub label: String,
    pub risk: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recap {
    pub day: String,
    pub sales_units: f64,
    pub p50_units: f64,
    pub delta_pct: f64,
    pub traffic: f64,
    pub traffic_vs_7d_pct: f64,
    pub top_miss_item: i32,
    pub top_miss_label: String,
    pub top_miss_actual: f64,
    pub top_miss_p50: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BriefTotals {
    pub onhand_units: f64,
    pub at_risk_units: f64,
    pub expected_sales_today: f64,
    pub order_lines: usize,
    pub order_units: i64,
    pub markdown_lines: usize,
    pub recovered_usd: f64,
    pub waste_avoided_usd: f64,
    pub promo_lines: usize,
    pub expected_incremental_units: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BriefFacts {
    pub store: i16,
    pub date: String,
    pub weekday: String,
    pub generated_for: String,
    pub store_meta: Store,
    pub holiday_today: Option<String>,
    pub holiday_tomorrow: Option<String>,
    pub service_level: f64,
    pub recap: Recap,
    pub totals: BriefTotals,
    pub orders: Vec<OrderLine>,
    pub markdowns: Vec<MarkdownLine>,
    pub promos: Vec<PromoLine>,
    pub watchlist: Vec<WatchLine>,
    pub assumptions: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromoCreative {
    pub item: i32,
    pub headline: String,
    pub body: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Narrative {
    pub headline: String,
    pub dek: String,
    pub orders_summary: String,
    pub markdowns_summary: String,
    pub promos_summary: String,
    pub watchlist_summary: String,
    #[serde(default)]
    pub promo_creative: Vec<PromoCreative>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BriefResponse {
    pub facts: BriefFacts,
    pub narrative: Option<Narrative>,
    pub grounding: Option<GroundingReport>,
    pub usage: Option<LlmUsage>,
    pub cost_usd: Option<f64>,
    pub model: String,
    pub source: String,
    pub generated_at: String,
    pub cached: bool,
}

fn why_chips(art: &ArtifactStore, store: i16, item: i32, k: usize) -> Vec<WhyChip> {
    art.attributions
        .get(&(store, item))
        .map(|v| {
            v.iter()
                .take(k)
                .map(|a| WhyChip { feature: a.feature.clone(), label: a.label.clone(), contrib: a.contrib })
                .collect()
        })
        .unwrap_or_default()
}

fn avg_tail(series: &[(NaiveDate, f64)], end: NaiveDate, days: i64) -> f64 {
    let from = end - chrono::Duration::days(days);
    let xs: Vec<f64> = series.iter().filter(|(d, _)| *d > from && *d <= end).map(|(_, v)| *v).collect();
    if xs.is_empty() { 0.0 } else { xs.iter().sum::<f64>() / xs.len() as f64 }
}

fn volatility(series: &[(NaiveDate, f64)], end: NaiveDate, days: i64) -> f64 {
    let from = end - chrono::Duration::days(days);
    let xs: Vec<f64> = series.iter().filter(|(d, _)| *d > from && *d <= end).map(|(_, v)| *v).collect();
    if xs.len() < 3 { return 0.0; }
    let mean = xs.iter().sum::<f64>() / xs.len() as f64;
    if mean <= 0.05 { return 0.0; }
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / xs.len() as f64;
    var.sqrt() / mean
}

/// Morning inventory state for a store at `date` (state after the previous day's close).
fn morning_state(art: &ArtifactStore, store: i16, date: NaiveDate) -> BTreeMap<i32, (f64, Option<NaiveDate>)> {
    let start = art.history_start + chrono::Duration::days(28);
    if date <= start {
        return BTreeMap::new();
    }
    let items = sim_items(art, store);
    let input = sim::build_input(store, items, start, date - chrono::Duration::days(1), SERVICE_LEVEL, 1.0, 1.0);
    let (_, states) = sim::run_with_state(&input, PolicyKind::Daymark);
    states.into_iter().map(|(i, s)| (i, (s.onhand, s.oldest_expiry))).collect()
}

/// Build simulator items for a store from the artifact tables.
pub fn sim_items(art: &ArtifactStore, store: i16) -> Vec<SimItem> {
    let mut items = Vec::new();
    for item_nbr in art.store_items(store) {
        let Some(item) = art.item(item_nbr) else { continue };
        let demand_series = art.demand_series(store, item_nbr);
        let demand: BTreeMap<NaiveDate, f64> = demand_series.iter().cloned().collect();
        let band: BTreeMap<NaiveDate, (f64, f64, f64)> = art
            .forecast
            .get(&store)
            .and_then(|m| m.get(&item_nbr))
            .map(|days| days.iter().map(|d| (d.date, (d.p10, d.p50, d.p90))).collect())
            .unwrap_or_default();
        let lift = art.family_lift_pct(&item.family).min(MARKDOWN_LIFT_CAP);
        items.push(SimItem {
            item: item_nbr,
            family: item.family.clone(),
            price: item.unit_price_usd,
            shelf_life_days: item.shelf_life_days as i32,
            lift_pct: lift,
            demand,
            band,
        });
    }
    items
}

pub fn build_facts(art: &ArtifactStore, store: i16, date: NaiveDate) -> Result<BriefFacts> {
    let store_meta = art
        .stores
        .get(&store)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("unknown store {store}"))?;
    let state = morning_state(art, store, date);
    let next = date + chrono::Duration::days(1);

    let mut orders = Vec::new();
    let mut markdowns = Vec::new();
    let mut promos = Vec::new();
    let mut watchlist = Vec::new();
    let mut onhand_total = 0.0;
    let mut at_risk_total = 0.0;
    let mut expected_sales_today = 0.0;

    for item_nbr in art.store_items(store) {
        let Some(item) = art.item(item_nbr) else { continue };
        let series = art.demand_series(store, item_nbr);
        let avg28 = avg_tail(&series, date, 28);
        let (onhand, oldest) = state.get(&item_nbr).copied().unwrap_or((0.0, None));
        onhand_total += onhand;
        let today_row = art.forecast_row(store, item_nbr, date);
        if let Some(r) = today_row {
            expected_sales_today += r.p50;
        }
        let tomorrow = art.forecast_row(store, item_nbr, next);

        // ---- orders (arrive tomorrow) ----
        if let Some(row) = tomorrow {
            let adv = order_advice(onhand, row.p10, row.p50, row.p90, SERVICE_LEVEL, item.shelf_life_days as i32, avg28);
            if adv.qty > 0 {
                orders.push(OrderLine {
                    item: item_nbr,
                    label: item.label(),
                    family: item.family.clone(),
                    onhand,
                    cover_days: if avg28 > 0.05 { onhand / avg28 } else { f64::INFINITY },
                    p10: row.p10,
                    p50: row.p50,
                    p90: row.p90,
                    suggest_qty: adv.qty,
                    unit_price_usd: item.unit_price_usd,
                    shelf_life_days: item.shelf_life_days,
                    promo_today: row.onpromotion,
                    why: why_chips(art, store, item_nbr, 3),
                });
            }
        }

        // ---- markdowns (expiring stock) ----
        if onhand > 0.5 {
            if let Some(exp) = oldest {
                let remaining = (exp - date).num_days();
                if let Some(cut) = markdown_cut(remaining) {
                    let boost = markdown_demand_boost(
                        art.family_lift_pct(&item.family).min(MARKDOWN_LIFT_CAP),
                        cut,
                        1.0,
                    );
                    let sell = today_row.map(|r| r.p50 * boost).unwrap_or(0.0);
                    let clear = onhand.min(sell.max(0.0));
                    let recovered = clear * item.unit_price_usd * (1.0 - cut);
                    let avoided = clear * item.unit_price_usd;
                    at_risk_total += onhand;
                    markdowns.push(MarkdownLine {
                        item: item_nbr,
                        label: item.label(),
                        family: item.family.clone(),
                        onhand,
                        expires_in_days: remaining,
                        cut_pct: cut,
                        expected_clear_units: clear,
                        recovered_usd: recovered,
                        waste_avoided_usd: avoided,
                        why: why_chips(art, store, item_nbr, 2),
                    });
                }
            }
        }

        // ---- promo candidates ----
        let lift_raw = art.family_lift_pct(&item.family);
        let lift = lift_raw.min(MARKDOWN_LIFT_CAP);
        if let (Some(row), true) = (tomorrow, lift >= 10.0) {
            let cover = if avg28 > 0.05 { onhand / avg28 } else { f64::INFINITY };
            let shelf = item.shelf_life_days as f64;
            if cover > shelf * 0.8 && row.p50 > 0.5 {
                let incremental = row.p50 * lift / 100.0;
                promos.push(PromoLine {
                    item: item_nbr,
                    label: item.label(),
                    family: item.family.clone(),
                    lift_pct: lift,
                    lift_measured_pct: lift_raw,
                    cover_days: cover,
                    onhand,
                    expected_incremental_units: incremental,
                    why: why_chips(art, store, item_nbr, 2),
                });
            }
        }

        // ---- watchlist ----
        if let Some(row) = tomorrow {
            let vol = volatility(&series, date, 28);
            if row.p90 >= 2.0 * row.p50 && onhand < row.p50 && row.p50 > 1.0 {
                watchlist.push(WatchLine {
                    item: item_nbr,
                    label: item.label(),
                    risk: "stockout".into(),
                    detail: format!(
                        "forecast {:.0}/day, upper band {:.0}, shelf stock {:.0}",
                        row.p50, row.p90, onhand
                    ),
                });
            } else if vol > 1.1 && avg28 > 1.0 {
                watchlist.push(WatchLine {
                    item: item_nbr,
                    label: item.label(),
                    risk: "volatile".into(),
                    detail: format!("week-to-week swing {:.0}% of average sales", vol * 100.0),
                });
            }
        }
    }

    orders.sort_by(|a, b| b.suggest_qty.cmp(&a.suggest_qty));
    markdowns.sort_by(|a, b| a.expires_in_days.cmp(&b.expires_in_days).then(b.onhand.partial_cmp(&a.onhand).unwrap_or(std::cmp::Ordering::Equal)));
    promos.sort_by(|a, b| b.expected_incremental_units.partial_cmp(&a.expected_incremental_units).unwrap_or(std::cmp::Ordering::Equal));
    watchlist.truncate(3);
    promos.truncate(4);

    // ---- recap of yesterday ----
    let yesterday = date - chrono::Duration::days(1);
    let mut sales_units = 0.0;
    let mut p50_units = 0.0;
    let mut top_miss: Option<(i32, String, f64, f64, f64)> = None;
    for item_nbr in art.store_items(store) {
        if let Some(r) = art.forecast_row(store, item_nbr, yesterday) {
            sales_units += r.actual;
            p50_units += r.p50;
            let miss = (r.actual - r.p50).abs();
            let better = top_miss.as_ref().map(|m| miss > m.4).unwrap_or(true);
            if better {
                if let Some(item) = art.item(item_nbr) {
                    top_miss = Some((item_nbr, item.label(), r.actual, r.p50, miss));
                }
            }
        }
    }
    let (traffic, traffic_prev) = {
        let today_cal: Option<&CalRow> = art.calendar.get(&(store, yesterday));
        let prev: Vec<f64> = (1..=7)
            .filter_map(|k| art.calendar.get(&(store, yesterday - chrono::Duration::days(k))).map(|c| c.transactions))
            .collect();
        let avg = if prev.is_empty() { 0.0 } else { prev.iter().sum::<f64>() / prev.len() as f64 };
        (today_cal.map(|c| c.transactions).unwrap_or(0.0), avg)
    };
    let (tm_item, tm_label, tm_actual, tm_p50) = top_miss
        .map(|(i, l, a, p, _)| (i, l, a, p))
        .unwrap_or((0, "—".to_string(), 0.0, 0.0));

    let recap = Recap {
        day: yesterday.to_string(),
        sales_units,
        p50_units,
        delta_pct: if p50_units > 0.0 { (sales_units - p50_units) / p50_units * 100.0 } else { 0.0 },
        traffic,
        traffic_vs_7d_pct: if traffic_prev > 0.0 { (traffic - traffic_prev) / traffic_prev * 100.0 } else { 0.0 },
        top_miss_item: tm_item,
        top_miss_label: tm_label,
        top_miss_actual: tm_actual,
        top_miss_p50: tm_p50,
    };

    let totals = BriefTotals {
        onhand_units: onhand_total,
        at_risk_units: at_risk_total,
        expected_sales_today,
        order_lines: orders.iter().filter(|o| o.suggest_qty > 0).count(),
        order_units: orders.iter().map(|o| o.suggest_qty as i64).sum(),
        markdown_lines: markdowns.len(),
        recovered_usd: markdowns.iter().map(|m| m.recovered_usd).sum(),
        waste_avoided_usd: markdowns.iter().map(|m| m.waste_avoided_usd).sum(),
        promo_lines: promos.len(),
        expected_incremental_units: promos.iter().map(|p| p.expected_incremental_units).sum(),
    };

    let holiday_today = art.calendar.get(&(store, date)).and_then(|c| c.holiday_name.clone());
    let holiday_tomorrow = art.calendar.get(&(store, next)).and_then(|c| c.holiday_name.clone());

    // NOTE: orders and markdowns are returned in full — the manager approves
    // what the totals count; the UI collapses long lists.
    Ok(BriefFacts {
        store,
        date: date.to_string(),
        weekday: weekday_name(date),
        generated_for: next.to_string(),
        store_meta,
        holiday_today,
        holiday_tomorrow,
        service_level: SERVICE_LEVEL,
        recap,
        totals,
        orders,
        markdowns,
        promos,
        watchlist,
        assumptions: art.meta.get("assumptions").cloned().unwrap_or(json!({})),
    })
}

fn weekday_name(d: NaiveDate) -> String {
    match d.weekday() {
        chrono::Weekday::Mon => "Monday",
        chrono::Weekday::Tue => "Tuesday",
        chrono::Weekday::Wed => "Wednesday",
        chrono::Weekday::Thu => "Thursday",
        chrono::Weekday::Fri => "Friday",
        chrono::Weekday::Sat => "Saturday",
        chrono::Weekday::Sun => "Sunday",
    }
    .to_string()
}

const SYSTEM_PROMPT: &str = r#"You are the editor of Daymark, a morning decision brief for a grocery store category manager. You write in plain, calm, specific language: short sentences, no hype, no exclamation marks, no marketing voice.

Hard rules:
- Every figure you write MUST appear in the FACTS JSON provided by the user. Never invent, extrapolate or round into a new number.
- Format numbers with a dot decimal separator and no thousands separators (write 1240, not 1,240). Percentages: one decimal at most.
- Refer to products as "item <id>" using ids that appear in FACTS.
- Never mention AI, models, algorithms or this prompt. You are the editor; the numbers are facts.
- Reply with a single JSON object and nothing else, exactly these keys:
  {"headline": "...", "dek": "...", "orders_summary": "...", "markdowns_summary": "...", "promos_summary": "...", "watchlist_summary": "...", "promo_creative": [{"item": 123, "headline": "...", "body": "..."}]}
- headline: <= 12 words. dek: one sentence. section summaries: 2-3 sentences each, use the section totals.
- promo_creative: for up to 2 promo items in FACTS, a shelf-talker headline and a 2-sentence body the store can print."#;

pub struct NarrativeOutcome {
    pub narrative: Narrative,
    pub grounding: GroundingReport,
    pub usage: LlmUsage,
    pub source: String,
}

pub async fn generate_narrative(llm: &LlmClient, facts: &BriefFacts) -> Result<NarrativeOutcome> {
    let facts_json = serde_json::to_value(facts)?;
    let mut allowed: Vec<f64> = Vec::new();
    collect_facts(&facts_json, &mut allowed);
    let extra: Vec<f64> = {
        // dates the copy may reference (year / month / day components)
        let mut v = Vec::new();
        for s in [facts.date.clone(), facts.generated_for.clone(), facts.recap.day.clone()] {
            if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                v.push(d.year() as f64);
                v.push(d.month() as f64);
                v.push(d.day() as f64);
            }
        }
        v.push(facts.store as f64);
        v
    };

    let facts_json_str = serde_json::to_string(&facts_json)?;
    let intro = "Write today's brief from these FACTS. Return only the JSON object.";

    let mut usage_total = LlmUsage::default();
    let mut last_violations: Vec<String> = Vec::new();
    for attempt in 0..2 {
        let prompt = if attempt == 0 {
            format!("{intro}\n\nFACTS:\n{facts_json_str}")
        } else {
            format!(
                "{intro}\n\nYour previous draft used numbers that are NOT in FACTS: {}. Rewrite using only numbers that appear in FACTS.\n\nFACTS:\n{facts_json_str}",
                last_violations.join(", ")
            )
        };
        let (raw, usage) = llm.chat(SYSTEM_PROMPT, &prompt, 900, 0.2).await?;
        usage_total.prompt_tokens += usage.prompt_tokens;
        usage_total.completion_tokens += usage.completion_tokens;
        usage_total.total_tokens += usage.total_tokens;

        let Ok(value) = extract_json(&raw) else {
            last_violations = vec!["unparseable JSON".into()];
            continue;
        };
        let Ok(narrative) = serde_json::from_value::<Narrative>(value) else {
            last_violations = vec!["missing required fields".into()];
            continue;
        };
        let mut report = verify_narrative(&narrative, &allowed, &extra);
        report.checked += 0;
        if report.ok {
            return Ok(NarrativeOutcome {
                narrative,
                grounding: report,
                usage: usage_total,
                source: if attempt == 0 { "model".into() } else { "model_retry".into() },
            });
        }
        last_violations = report.violations.clone();
    }

    // deterministic fallback — grounded by construction, still verified
    let narrative = template_narrative(facts);
    let report = verify_narrative(&narrative, &allowed, &extra);
    Ok(NarrativeOutcome {
        narrative,
        grounding: GroundingReport { source: "template_fallback".into(), ..report },
        usage: usage_total,
        source: "template_fallback".into(),
    })
}

pub fn verify_narrative(n: &Narrative, allowed: &[f64], extra: &[f64]) -> GroundingReport {
    let mut all = GroundingReport { ok: true, checked: 0, violations: vec![], source: "grounding_checker".into() };
    let mut texts = vec![
        n.headline.clone(),
        n.dek.clone(),
        n.orders_summary.clone(),
        n.markdowns_summary.clone(),
        n.promos_summary.clone(),
        n.watchlist_summary.clone(),
    ];
    for c in &n.promo_creative {
        texts.push(c.headline.clone());
        texts.push(c.body.clone());
    }
    for t in texts {
        let r = verify(&t, allowed, extra);
        all.checked += r.checked;
        all.violations.extend(r.violations);
    }
    all.ok = all.violations.is_empty();
    all
}

fn template_narrative(facts: &BriefFacts) -> Narrative {
    let t = &facts.totals;
    Narrative {
        headline: format!(
            "{} orders, {} markdowns due for {}",
            t.order_lines,
            t.markdown_lines,
            facts.generated_for
        ),
        dek: format!(
            "{} units on hand, {} at risk of expiry, {} units expected out today.",
            t.onhand_units.round(),
            t.at_risk_units.round(),
            t.expected_sales_today.round()
        ),
        orders_summary: format!(
            "Place {} order lines totalling {} units for tomorrow. Expected sales today: {} units. Shelf stock: {} units.",
            t.order_lines,
            t.order_units,
            t.expected_sales_today.round(),
            t.onhand_units.round()
        ),
        markdowns_summary: format!(
            "{} lines are inside their last two days: {} units at risk. Marking them down clears an expected {} units and avoids {} of thrown stock at full price.",
            t.markdown_lines,
            t.at_risk_units.round(),
            t.recovered_usd.round(),
            t.waste_avoided_usd.round()
        ),
        promos_summary: format!(
            "{} promo candidates with measured family lift; expected {} incremental units tomorrow.",
            t.promo_lines,
            t.expected_incremental_units.round()
        ),
        watchlist_summary: format!("{} items flagged for stockout or volatility.", facts.watchlist.len()),
        promo_creative: facts
            .promos
            .iter()
            .take(2)
            .map(|p| PromoCreative {
                item: p.item,
                headline: format!("{} · today only", p.family),
                body: format!(
                    "Measured lift {:.0}% on this category. Shelf stock covers {:.0} days.",
                    p.lift_pct, p.cover_days
                ),
            })
            .collect(),
    }
}
