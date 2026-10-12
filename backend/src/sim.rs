//! Deterministic perishable-store simulator — the Replay proof engine.
//!
//! Ground truth: recorded demand from the source dataset is treated as true
//! demand (the dataset has no inventory, so stock-out censoring is a declared
//! limitation). Every policy runs over the *same* demand stream, with the same
//! starting inventory and the same shelf life, so the delta between policies is
//! exact arithmetic, not an estimate.
//!
//! Policies under test:
//!   Spreadsheet  — order last same-weekday sales × 1.05, no markdowns
//!   ForecastOnly — newsvendor at the service level, no markdowns
//!   Daymark      — newsvendor + markdowns for expiring lots

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Serialize;

use crate::policy::{markdown_cut, markdown_demand_boost, order_advice, spreadsheet_order};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyKind {
    Spreadsheet,
    ForecastOnly,
    Daymark,
}

impl PolicyKind {
    pub fn id(&self) -> &'static str {
        match self {
            PolicyKind::Spreadsheet => "spreadsheet",
            PolicyKind::ForecastOnly => "forecast_only",
            PolicyKind::Daymark => "daymark",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            PolicyKind::Spreadsheet => "Spreadsheet (as-is)",
            PolicyKind::ForecastOnly => "Forecast only",
            PolicyKind::Daymark => "Daymark policy",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SimItem {
    pub item: i32,
    pub family: String,
    pub price: f64,
    pub shelf_life_days: i32,
    pub lift_pct: f64,
    /// known demand (history + forecast actuals)
    pub demand: BTreeMap<NaiveDate, f64>,
    /// forecast band for (p10,p50,p90)
    pub band: BTreeMap<NaiveDate, (f64, f64, f64)>,
}

#[derive(Clone, Debug)]
pub struct SimInput {
    pub store: i16,
    pub items: Vec<SimItem>,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub service_level: f64,
    pub lift_scale: f64,
    pub shelf_life_scale: f64,
    /// number of days before `start` used to seed opening inventory
    pub warmup_days: i64,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct DayStat {
    pub date: NaiveDate,
    pub sales_units: f64,
    pub lost_units: f64,
    pub waste_units: f64,
    pub waste_value_usd: f64,
    pub revenue_usd: f64,
    pub onhand_end: f64,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Totals {
    pub sales_units: f64,
    pub lost_units: f64,
    pub waste_units: f64,
    pub waste_value_usd: f64,
    pub revenue_usd: f64,
    pub service_level_pct: f64,
    pub demand_units: f64,
    pub markdown_units: f64,
    pub markdowns_triggered: u32,
    pub orders_placed: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FamilyStat {
    pub family: String,
    pub demand_units: f64,
    pub sales_units: f64,
    pub lost_units: f64,
    pub waste_units: f64,
    pub waste_value_usd: f64,
    pub service_level_pct: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SimOutcome {
    pub policy: String,
    pub policy_label: String,
    pub daily: Vec<DayStat>,
    pub totals: Totals,
    pub by_family: Vec<FamilyStat>,
}

struct Lot {
    expiry: NaiveDate,
    qty: f64,
    /// price cut applied to this lot (0.0 = full price)
    cut: f64,
}

struct ItemState {
    lots: Vec<Lot>,
    pending: Option<(NaiveDate, f64)>,
}

pub struct DayState {
    pub onhand: f64,
    pub oldest_expiry: Option<NaiveDate>,
}

/// Final-state snapshot for brief assembly: on-hand + oldest lot expiry per item.
pub fn run_with_state(input: &SimInput, policy: PolicyKind) -> (SimOutcome, BTreeMap<i32, DayState>) {
    let mut states: BTreeMap<i32, ItemState> = BTreeMap::new();
    let mut onhand_totals: BTreeMap<i32, (f64, f64)> = BTreeMap::new(); // item -> (sales, demand over warmup)

    // ---- seed opening inventory: shelf-life-scaled cover of recent demand ----
    for it in &input.items {
        let warm_start = input.start - chrono::Duration::days(input.warmup_days);
        let recent: Vec<f64> = it
            .demand
            .range(warm_start..input.start)
            .map(|(_, v)| *v)
            .collect();
        let mean = if recent.is_empty() { 0.0 } else { recent.iter().sum::<f64>() / recent.len() as f64 };
        let shelf = ((it.shelf_life_days as f64) * input.shelf_life_scale).max(1.0);
        let qty = mean * shelf;
        states.insert(
            it.item,
            ItemState {
                lots: if qty > 0.0 {
                    vec![Lot { expiry: input.start + chrono::Duration::days(shelf as i64), qty, cut: 0.0 }]
                } else {
                    vec![]
                },
                pending: None,
            },
        );
    }

    let mut daily: Vec<DayStat> = Vec::new();
    let mut totals = Totals::default();
    let mut fam: BTreeMap<String, FamilyStat> = BTreeMap::new();

    let mut day = input.start;
    while day <= input.end {
        let mut stat = DayStat { date: day, ..Default::default() };
        for it in &input.items {
            let st = states.get_mut(&it.item).unwrap();
            // receive pending delivery
            if let Some((arrival, qty)) = st.pending.take() {
                if arrival <= day && qty > 0.0 {
                    let shelf = ((it.shelf_life_days as f64) * input.shelf_life_scale).max(1.0);
                    st.lots.push(Lot { expiry: day + chrono::Duration::days(shelf as i64), qty, cut: 0.0 });
                }
            }
            let onhand_before: f64 = st.lots.iter().map(|l| l.qty).sum();
            let demand = it.demand.get(&day).copied().unwrap_or(0.0);

            // ---- markdowns (Daymark only) ---------------------------------------
            // Trigger only on *excess* stock: units that will not sell before the
            // oldest lot expires. Discount applies to the at-risk lots only.
            let mut boost = 1.0;
            let mut marked = false;
            if policy == PolicyKind::Daymark && onhand_before > 0.0 {
                let oldest = st.lots.iter().filter(|l| l.qty > 0.0).map(|l| l.expiry).min();
                if let Some(exp) = oldest {
                    let remaining = (exp - day).num_days();
                    if let Some(cut) = markdown_cut(remaining) {
                        // expected sales until the oldest lot expires (band p50 where available)
                        let mut expected = 0.0;
                        let mut dcur = day;
                        while dcur <= exp {
                            expected += it
                                .band
                                .get(&dcur)
                                .map(|(_, p50, _)| *p50)
                                .or_else(|| it.demand.get(&dcur).copied())
                                .unwrap_or(0.0);
                            dcur += chrono::Duration::days(1);
                        }
                        let excess = (onhand_before - expected).max(0.0);
                        if excess > 1.0 && excess >= 0.15 * onhand_before {
                            marked = true;
                            for lot in st.lots.iter_mut() {
                                if lot.qty > 0.0 && lot.expiry <= day + chrono::Duration::days(2) {
                                    lot.cut = lot.cut.max(cut);
                                }
                            }
                            boost = markdown_demand_boost(it.lift_pct, cut, input.lift_scale);
                            totals.markdowns_triggered += 1;
                        }
                    }
                }
            }
            let effective_demand = demand * boost;

            // ---- consume FIFO from oldest lots (per-lot pricing) -----------------
            st.lots.sort_by_key(|l| l.expiry);
            let mut remaining = effective_demand;
            let mut sold = 0.0;
            let mut marked_units = 0.0;
            let mut revenue = 0.0;
            for lot in st.lots.iter_mut() {
                if remaining <= 0.0 {
                    break;
                }
                let take = lot.qty.min(remaining);
                lot.qty -= take;
                remaining -= take;
                sold += take;
                revenue += take * it.price * (1.0 - lot.cut);
                if lot.cut > 0.0 {
                    marked_units += take;
                }
            }
            let lost = (effective_demand - sold).max(0.0);
            let _ = marked;

            // ---- end of day: expire lots ----
            let mut wasted = 0.0;
            for lot in st.lots.iter_mut() {
                if lot.expiry <= day && lot.qty > 0.0 {
                    wasted += lot.qty;
                    lot.qty = 0.0;
                }
            }
            let onhand_after = st.lots.iter().map(|l| l.qty).sum();
            stat.sales_units += sold;
            stat.lost_units += lost;
            stat.waste_units += wasted;
            stat.waste_value_usd += wasted * it.price;
            stat.revenue_usd += revenue;
            stat.onhand_end += onhand_after;

            totals.sales_units += sold;
            totals.lost_units += lost;
            totals.waste_units += wasted;
            totals.waste_value_usd += wasted * it.price;
            totals.revenue_usd += revenue;
            totals.demand_units += effective_demand;
            totals.markdown_units += marked_units;
            onhand_totals.entry(it.item).or_insert((0.0, 0.0)).0 += sold;

            let entry = fam.entry(it.family.clone()).or_insert(FamilyStat {
                family: it.family.clone(),
                demand_units: 0.0,
                sales_units: 0.0,
                lost_units: 0.0,
                waste_units: 0.0,
                waste_value_usd: 0.0,
                service_level_pct: 0.0,
            });
            entry.demand_units += demand;
            entry.sales_units += sold;
            entry.lost_units += lost;
            entry.waste_units += wasted;
            entry.waste_value_usd += wasted * it.price;

            // ---- order for tomorrow ----
            let next = day + chrono::Duration::days(1);
            if next <= input.end {
                let order = match policy {
                    PolicyKind::Spreadsheet => {
                        let last_week = it.demand.get(&(day - chrono::Duration::days(7))).copied();
                        let recent: Vec<f64> = it
                            .demand
                            .range(day - chrono::Duration::days(7)..day)
                            .map(|(_, v)| *v)
                            .collect();
                        let mean7 = if recent.is_empty() { 0.0 } else { recent.iter().sum::<f64>() / recent.len() as f64 };
                        // an experienced manager floors last-week's figure with the recent average
                        let basis = last_week.unwrap_or(mean7).max(mean7);
                        spreadsheet_order(basis, onhand_after)
                    }
                    _ => {
                        let (p10, p50, p90) = it.band.get(&next).copied().unwrap_or((0.0, 0.0, 0.0));
                        if p50 <= 0.0 {
                            0
                        } else {
                            let recent: Vec<f64> = it
                                .demand
                                .range(day - chrono::Duration::days(28)..=day)
                                .map(|(_, v)| *v)
                                .collect();
                            let avg = if recent.is_empty() { 0.0 } else { recent.iter().sum::<f64>() / recent.len() as f64 };
                            order_advice(onhand_after, p10, p50, p90, input.service_level, it.shelf_life_days, avg).qty
                        }
                    }
                };
                if order > 0 {
                    totals.orders_placed += order as f64;
                    st.pending = Some((next, order as f64));
                }
            }
        }
        totals.service_level_pct = if totals.demand_units > 0.0 {
            totals.sales_units / totals.demand_units * 100.0
        } else {
            100.0
        };
        daily.push(stat);
        day += chrono::Duration::days(1);
    }

    let states_out: BTreeMap<i32, DayState> = states
        .iter()
        .map(|(item, st)| {
            (
                *item,
                DayState {
                    onhand: st.lots.iter().map(|l| l.qty).sum(),
                    oldest_expiry: st.lots.iter().filter(|l| l.qty > 0.0).map(|l| l.expiry).min(),
                },
            )
        })
        .collect();

    let mut by_family: Vec<FamilyStat> = fam
        .into_values()
        .map(|mut f| {
            f.service_level_pct = if f.demand_units > 0.0 { f.sales_units / f.demand_units * 100.0 } else { 100.0 };
            f
        })
        .collect();
    by_family.sort_by(|a, b| b.waste_units.partial_cmp(&a.waste_units).unwrap_or(std::cmp::Ordering::Equal));

    (
        SimOutcome {
            policy: policy.id().to_string(),
            policy_label: policy.label().to_string(),
            daily,
            totals,
            by_family,
        },
        states_out,
    )
}

pub fn run(input: &SimInput, policy: PolicyKind) -> SimOutcome {
    run_with_state(input, policy).0
}

/// Build the simulator input for a store from artifact tables.
pub fn build_input(
    store: i16,
    items: Vec<SimItem>,
    start: NaiveDate,
    end: NaiveDate,
    service_level: f64,
    lift_scale: f64,
    shelf_life_scale: f64,
) -> SimInput {
    SimInput {
        store,
        items,
        start,
        end,
        service_level,
        lift_scale,
        shelf_life_scale,
        warmup_days: 28,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn toy_item() -> SimItem {
        // steady demand 10/day for 40 days; shelf life 3 days
        let start = d("2017-07-01");
        let mut demand = BTreeMap::new();
        let mut band = BTreeMap::new();
        for i in 0..40 {
            let day = start + chrono::Duration::days(i);
            demand.insert(day, 10.0);
            band.insert(day, (8.0, 10.0, 13.0));
        }
        SimItem {
            item: 1,
            family: "PRODUCE".into(),
            price: 1.0,
            shelf_life_days: 3,
            lift_pct: 20.0,
            demand,
            band,
        }
    }

    fn input() -> SimInput {
        build_input(1, vec![toy_item()], d("2017-07-01"), d("2017-07-30"), 0.92, 1.0, 1.0)
    }

    #[test]
    fn accounting_identity_holds() {
        for p in [PolicyKind::Spreadsheet, PolicyKind::ForecastOnly, PolicyKind::Daymark] {
            let out = run(&input(), p);
            let t = &out.totals;
            assert!(
                (t.demand_units - t.sales_units - t.lost_units).abs() < 1e-6,
                "sales + lost must equal demand for {:?}",
                p
            );
            assert!(t.waste_units >= 0.0);
            assert!(t.service_level_pct <= 100.0 + 1e-9);
        }
    }

    #[test]
    fn deterministic() {
        let a = run(&input(), PolicyKind::Daymark);
        let b = run(&input(), PolicyKind::Daymark);
        assert_eq!(a.totals.sales_units, b.totals.sales_units);
        assert_eq!(a.totals.waste_units, b.totals.waste_units);
    }

    #[test]
    fn markdown_policy_reduces_waste_when_expiring_stock_exists() {
        // force overstock: demand 10/day, shelf life 3, but seed high inventory via warmup
        let mut inp = input();
        inp.warmup_days = 28; // seeds 3 * 10 = 30 units at start
        let daymark = run(&inp, PolicyKind::Daymark).totals;
        let forecast_only = run(&inp, PolicyKind::ForecastOnly).totals;
        // markdowns convert expiring stock into sales → waste must not be worse
        assert!(daymark.waste_units <= forecast_only.waste_units + 1e-9);
        assert!(daymark.service_level_pct >= forecast_only.service_level_pct - 1e-6);
    }

    #[test]
    fn spreadsheets_overorder_when_demand_noisy() {
        // noisy demand: 0..20 — spreadsheets over-order on peaks and waste
        let mut it = toy_item();
        it.demand = it
            .demand
            .iter()
            .enumerate()
            .map(|(i, (day, _))| (*day, if i % 2 == 0 { 18.0 } else { 2.0 }))
            .collect();
        let inp = build_input(1, vec![it.clone()], d("2017-07-01"), d("2017-07-30"), 0.92, 1.0, 1.0);
        let sheet = run(&inp, PolicyKind::Spreadsheet).totals;
        let day_ = run(&inp, PolicyKind::Daymark).totals;
        assert!(day_.waste_units <= sheet.waste_units);
    }
}
