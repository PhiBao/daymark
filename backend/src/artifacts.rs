//! Load the engine's serving artifacts into typed in-memory tables.
//!
//! Contract (engine/artifacts/): meta.json, eval_metrics.json, promo_lift.json,
//! forecast.parquet, attributions.parquet, items.parquet, stores.parquet,
//! calendar.parquet, history_window.parquet, optional benchmark.json.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::NaiveDate;
use polars::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
pub struct Item {
    pub item_nbr: i32,
    pub family: String,
    pub class: i16,
    pub perishable: bool,
    pub shelf_life_days: i16,
    pub unit_price_usd: f64,
}

impl Item {
    /// Display alias — the source dataset anonymizes items, we stay honest about it.
    pub fn label(&self) -> String {
        format!("{} · item {}", title_case(&self.family), self.item_nbr)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Store {
    pub store_nbr: i16,
    pub city: String,
    pub state: String,
    pub store_type: String,
    pub cluster: i16,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForecastRow {
    pub date: NaiveDate,
    pub p10: f64,
    pub p50: f64,
    pub p90: f64,
    pub actual: f64,
    pub onpromotion: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Attribution {
    pub feature: String,
    pub label: String,
    pub contrib: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CalRow {
    pub holiday_name: Option<String>,
    pub is_local_holiday: bool,
    pub is_weekend: bool,
    pub transactions: f64,
    pub oil: f64,
}

pub struct ArtifactStore {
    pub meta: Value,
    pub eval_metrics: Value,
    pub promo_lift: Value,
    pub benchmark: Option<Value>,
    pub items: HashMap<i32, Item>,
    pub stores: BTreeMap<i16, Store>,
    /// store -> item -> days (sorted ascending)
    pub forecast: HashMap<i16, HashMap<i32, Vec<ForecastRow>>>,
    pub attributions: HashMap<(i16, i32), Vec<Attribution>>,
    pub calendar: HashMap<(i16, NaiveDate), CalRow>,
    /// daily sales before the forecast window (warm-up for the simulator)
    pub history: HashMap<(i16, i32), Vec<(NaiveDate, f64)>>,
    pub families: Vec<String>,
    pub forecast_start: NaiveDate,
    pub forecast_end: NaiveDate,
    pub history_start: NaiveDate,
}

fn col_f64(df: &DataFrame, name: &str) -> Result<Vec<Option<f64>>> {
    let s = df.column(name)?.cast(&DataType::Float64)?;
    Ok(s.f64()?.iter().collect())
}
fn col_i32(df: &DataFrame, name: &str) -> Result<Vec<Option<i32>>> {
    let s = df.column(name)?.cast(&DataType::Int32)?;
    Ok(s.i32()?.iter().collect())
}
fn col_i16(df: &DataFrame, name: &str) -> Result<Vec<Option<i16>>> {
    let s = df.column(name)?.cast(&DataType::Int16)?;
    Ok(s.i16()?.iter().collect())
}
fn col_bool(df: &DataFrame, name: &str) -> Result<Vec<Option<bool>>> {
    let s = df.column(name)?;
    if s.dtype() == &DataType::Boolean {
        return Ok(s.bool()?.iter().collect());
    }
    // int-encoded boolean (0/1)
    let s = s.cast(&DataType::Int8)?;
    Ok(s.i8()?.iter().map(|o| o.map(|v| v != 0)).collect())
}
fn col_str(df: &DataFrame, name: &str) -> Result<Vec<Option<String>>> {
    let s = df.column(name)?.cast(&DataType::String)?;
    Ok(s.str()?.iter().map(|o| o.map(|v| v.to_string())).collect())
}
fn col_date(df: &DataFrame, name: &str) -> Result<Vec<Option<NaiveDate>>> {
    let raw = col_str(df, name)?;
    raw.into_iter()
        .map(|o| match o {
            None => Ok(None),
            Some(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                .map(Some)
                .with_context(|| format!("bad date {s} in {name}")),
        })
        .collect()
}

fn read_json(path: &Path) -> Result<Value> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_str(&raw)?)
}

fn read_parquet(path: &Path) -> Result<DataFrame> {
    let f = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    Ok(ParquetReader::new(f).finish()?)
}

pub fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl ArtifactStore {
    pub fn load(dir: &Path) -> Result<Self> {
        let meta = read_json(&dir.join("meta.json"))?;
        let eval_metrics = read_json(&dir.join("eval_metrics.json"))?;
        let promo_lift = read_json(&dir.join("promo_lift.json"))?;
        let benchmark = if dir.join("benchmark.json").exists() {
            Some(read_json(&dir.join("benchmark.json"))?)
        } else {
            None
        };

        // ---- items ----
        let idf = read_parquet(&dir.join("items.parquet"))?;
        let (i_nbr, i_fam, i_cls, i_per, i_shelf, i_price) = (
            col_i32(&idf, "item_nbr")?,
            col_str(&idf, "family")?,
            col_i16(&idf, "class")?,
            col_bool(&idf, "perishable")?,
            col_i16(&idf, "shelf_life_days")?,
            col_f64(&idf, "unit_price_usd")?,
        );
        let mut items = HashMap::new();
        for idx in 0..idf.height() {
            let item_nbr = i_nbr[idx].unwrap_or_default();
            items.insert(
                item_nbr,
                Item {
                    item_nbr,
                    family: i_fam[idx].clone().unwrap_or_default(),
                    class: i_cls[idx].unwrap_or_default(),
                    perishable: i_per[idx].unwrap_or(false),
                    shelf_life_days: i_shelf[idx].unwrap_or(4),
                    unit_price_usd: i_price[idx].unwrap_or(2.5),
                },
            );
        }

        // ---- stores ----
        let sdf = read_parquet(&dir.join("stores.parquet"))?;
        let (s_nbr, s_city, s_state, s_type, s_cluster) = (
            col_i16(&sdf, "store_nbr")?,
            col_str(&sdf, "city")?,
            col_str(&sdf, "state")?,
            col_str(&sdf, "type")?,
            col_i16(&sdf, "cluster")?,
        );
        let mut stores = BTreeMap::new();
        for idx in 0..sdf.height() {
            let store_nbr = s_nbr[idx].unwrap_or_default();
            stores.insert(
                store_nbr,
                Store {
                    store_nbr,
                    city: s_city[idx].clone().unwrap_or_default(),
                    state: s_state[idx].clone().unwrap_or_default(),
                    store_type: s_type[idx].clone().unwrap_or_default(),
                    cluster: s_cluster[idx].unwrap_or_default(),
                },
            );
        }

        // ---- forecast ----
        let fdf = read_parquet(&dir.join("forecast.parquet"))?;
        let (f_store, f_item, f_date, f_p10, f_p50, f_p90, f_act, f_promo) = (
            col_i16(&fdf, "store_nbr")?,
            col_i32(&fdf, "item_nbr")?,
            col_date(&fdf, "date")?,
            col_f64(&fdf, "p10")?,
            col_f64(&fdf, "p50")?,
            col_f64(&fdf, "p90")?,
            col_f64(&fdf, "unit_sales")?,
            col_bool(&fdf, "onpromotion")?,
        );
        let mut forecast: HashMap<i16, HashMap<i32, Vec<ForecastRow>>> = HashMap::new();
        let mut fmin = NaiveDate::MAX;
        let mut fmax = NaiveDate::MIN;
        for idx in 0..fdf.height() {
            let (Some(store), Some(item), Some(date)) = (f_store[idx], f_item[idx], f_date[idx]) else {
                continue;
            };
            fmin = fmin.min(date);
            fmax = fmax.max(date);
            forecast.entry(store).or_default().entry(item).or_default().push(ForecastRow {
                date,
                p10: f_p10[idx].unwrap_or(0.0),
                p50: f_p50[idx].unwrap_or(0.0),
                p90: f_p90[idx].unwrap_or(0.0),
                actual: f_act[idx].unwrap_or(0.0),
                onpromotion: f_promo[idx].unwrap_or(false),
            });
        }
        for per_item in forecast.values_mut() {
            for days in per_item.values_mut() {
                days.sort_by_key(|d| d.date);
            }
        }

        // ---- attributions ----
        let adf = read_parquet(&dir.join("attributions.parquet"))?;
        let (a_store, a_item, a_feat, a_label, a_contrib) = (
            col_i16(&adf, "store_nbr")?,
            col_i32(&adf, "item_nbr")?,
            col_str(&adf, "feature")?,
            col_str(&adf, "label")?,
            col_f64(&adf, "contrib")?,
        );
        let mut attributions: HashMap<(i16, i32), Vec<Attribution>> = HashMap::new();
        for idx in 0..adf.height() {
            let (Some(store), Some(item)) = (a_store[idx], a_item[idx]) else { continue };
            attributions.entry((store, item)).or_default().push(Attribution {
                feature: a_feat[idx].clone().unwrap_or_default(),
                label: a_label[idx].clone().unwrap_or_default(),
                contrib: a_contrib[idx].unwrap_or(0.0),
            });
        }
        for v in attributions.values_mut() {
            v.sort_by(|a, b| b.contrib.abs().partial_cmp(&a.contrib.abs()).unwrap_or(std::cmp::Ordering::Equal));
        }

        // ---- calendar ----
        let cdf = read_parquet(&dir.join("calendar.parquet"))?;
        let (c_store, c_date, c_hol, c_lhol, c_wknd, c_tx, c_oil) = (
            col_i16(&cdf, "store_nbr")?,
            col_date(&cdf, "date")?,
            col_str(&cdf, "holiday_name")?,
            col_bool(&cdf, "is_local_holiday")?,
            col_bool(&cdf, "is_weekend")?,
            col_f64(&cdf, "transactions")?,
            col_f64(&cdf, "dcoilwtico")?,
        );
        let mut calendar = HashMap::new();
        for idx in 0..cdf.height() {
            let (Some(store), Some(date)) = (c_store[idx], c_date[idx]) else { continue };
            calendar.insert(
                (store, date),
                CalRow {
                    holiday_name: c_hol[idx].clone(),
                    is_local_holiday: c_lhol[idx].unwrap_or(false),
                    is_weekend: c_wknd[idx].unwrap_or(false),
                    transactions: c_tx[idx].unwrap_or(0.0),
                    oil: c_oil[idx].unwrap_or(0.0),
                },
            );
        }

        // ---- history window (simulator warm-up + "recent sales") ----
        let hdf = read_parquet(&dir.join("history_window.parquet"))?;
        let (h_store, h_item, h_date, h_sales) = (
            col_i16(&hdf, "store_nbr")?,
            col_i32(&hdf, "item_nbr")?,
            col_date(&hdf, "date")?,
            col_f64(&hdf, "unit_sales")?,
        );
        let mut history: HashMap<(i16, i32), Vec<(NaiveDate, f64)>> = HashMap::new();
        let mut hmin = NaiveDate::MAX;
        for idx in 0..hdf.height() {
            let (Some(store), Some(item), Some(date)) = (h_store[idx], h_item[idx], h_date[idx]) else {
                continue;
            };
            hmin = hmin.min(date);
            history.entry((store, item)).or_default().push((date, h_sales[idx].unwrap_or(0.0)));
        }
        for v in history.values_mut() {
            v.sort_by_key(|(d, _)| *d);
        }

        let mut families: Vec<String> = items.values().map(|i| i.family.clone()).collect();
        families.sort();
        families.dedup();

        Ok(Self {
            meta,
            eval_metrics,
            promo_lift,
            benchmark,
            items,
            stores,
            forecast,
            attributions,
            calendar,
            history,
            families,
            forecast_start: fmin,
            forecast_end: fmax,
            history_start: hmin,
        })
    }

    pub fn item(&self, item_nbr: i32) -> Option<&Item> {
        self.items.get(&item_nbr)
    }

    /// Sorted list of items carried by a store.
    pub fn store_items(&self, store: i16) -> Vec<i32> {
        let mut v: Vec<i32> = self
            .forecast
            .get(&store)
            .map(|m| m.keys().copied().collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    /// Demand series (history + forecast actuals merged, ascending) for simulator warm-up.
    pub fn demand_series(&self, store: i16, item: i32) -> Vec<(NaiveDate, f64)> {
        let mut series: Vec<(NaiveDate, f64)> = self
            .history
            .get(&(store, item))
            .cloned()
            .unwrap_or_default();
        if let Some(days) = self.forecast.get(&store).and_then(|m| m.get(&item)) {
            for d in days {
                series.push((d.date, d.actual));
            }
        }
        series.sort_by_key(|(d, _)| *d);
        series.dedup_by_key(|(d, _)| *d);
        series
    }

    pub fn forecast_row(&self, store: i16, item: i32, date: NaiveDate) -> Option<&ForecastRow> {
        self.forecast
            .get(&store)?
            .get(&item)?
            .iter()
            .find(|r| r.date == date)
    }

    pub fn family_lift_pct(&self, family: &str) -> f64 {
        self.promo_lift
            .get("by_family")
            .and_then(|f| f.get(family))
            .and_then(|f| f.get("median_pct"))
            .and_then(|v| v.as_f64())
            .or_else(|| self.promo_lift.get("overall_pct").and_then(|v| v.as_f64()))
            .unwrap_or(10.0)
    }

    pub fn artifacts_dir(&self) -> Option<PathBuf> {
        None
    }
}
