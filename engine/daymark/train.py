"""Train the Daymark demand forecaster and export serving artifacts.

Model: LightGBM quantile regression (p10 / p50 / p90) on log1p(unit_sales).
Everything is evaluated strictly out-of-sample on the backtest window
(2017-06-01 .. 2017-08-15), which includes the demo brief date.

Artifacts written to engine/artifacts/:
  forecast.parquet     store,item,date,p10,p50,p90,unit_sales,onpromotion,family
  attributions.parquet store,item,date,rank,feature,label,contrib  (brief date, top-5)
  eval_metrics.json    WAPE / pinball / coverage + baselines, overall + by family
  promo_lift.json      measured on-promotion lift per family from real history
  model/               LightGBM boosters (txt) + feature list (provenance)
"""

from __future__ import annotations

import json
import math

import lightgbm as lgb
import numpy as np
import polars as pl

from daymark.config import (
    ARTIFACTS,
    BACKTEST_END,
    BACKTEST_START,
    BRIEF_DATE,
    INTERIM,
    TRAIN_END,
)
from daymark.features import CAT_FEATURES, NUM_FEATURES, TARGET, build_features

ALPHAS = {"p10": 0.1, "p50": 0.5, "p90": 0.9}

FEATURE_LABELS = {
    "sales_lag_1": "yesterday",
    "sales_lag_7": "same weekday last week",
    "sales_lag_14": "two weeks ago",
    "sales_lag_28": "four weeks ago",
    "sales_mean_7": "7-day average",
    "sales_mean_14": "14-day average",
    "sales_mean_28": "28-day average",
    "sales_median_28": "28-day median",
    "sales_std_7": "volatility (7d)",
    "sales_std_28": "volatility (28d)",
    "promo_count_7": "promos in the last week",
    "onpromotion": "on promotion today",
    "dow": "day of week",
    "is_weekend": "weekend",
    "month": "season",
    "dom": "day of month",
    "woy": "week of year",
    "is_holiday": "public holiday",
    "is_local_holiday": "local holiday",
    "is_mothers_day": "Mother's Day",
    "is_payday": "payday week",
    "transactions": "store traffic",
    "transactions_mean_7": "store traffic (7d)",
    "dcoilwtico": "oil price",
    "family": "category",
    "class": "item class",
    "store_type": "store type",
    "cluster": "store cluster",
    "store_nbr": "store identity",
    "item_nbr": "item identity",
}


def _pinball(y: np.ndarray, yhat: np.ndarray, alpha: float) -> float:
    d = y - yhat
    return float(np.mean(np.maximum(alpha * d, (alpha - 1) * d)))


def _wape(y: np.ndarray, yhat: np.ndarray) -> float:
    denom = float(np.sum(np.abs(y)))
    return float(np.sum(np.abs(y - yhat)) / denom) if denom > 0 else math.nan


def _as_pandas(df: pl.DataFrame) -> "object":
    X = df.select(NUM_FEATURES + CAT_FEATURES).to_pandas()
    for c in CAT_FEATURES:
        X[c] = X[c].astype("category")
    return X


def _train_models(df: pl.DataFrame) -> dict[str, lgb.Booster]:
    train = df.filter(pl.col("date") <= pl.lit(TRAIN_END).str.to_date())
    val_cut = train.select(pl.col("date").max()).item() - __import__("datetime").timedelta(days=35)
    tr = train.filter(pl.col("date") < val_cut)
    va = train.filter(pl.col("date") >= val_cut)
    X_tr, y_tr = _as_pandas(tr), np.log1p(tr[TARGET].to_numpy())
    X_va, y_va = _as_pandas(va), np.log1p(va[TARGET].to_numpy())
    print(f"  train rows={len(tr):,} val rows={len(va):,} (val from {val_cut})", flush=True)

    params = dict(
        learning_rate=0.05,
        num_leaves=96,
        min_data_in_leaf=40,
        feature_fraction=0.9,
        bagging_fraction=0.9,
        bagging_freq=1,
        lambda_l2=1.0,
        verbosity=-1,
        num_threads=0,
        objective="quantile",
    )
    models: dict[str, lgb.Booster] = {}
    for name, alpha in ALPHAS.items():
        dtr = lgb.Dataset(X_tr, y_tr, categorical_feature=CAT_FEATURES, free_raw_data=False)
        dva = lgb.Dataset(X_va, y_va, reference=dtr, categorical_feature=CAT_FEATURES, free_raw_data=False)
        booster = lgb.train(
            dict(params, alpha=alpha), dtr, num_boost_round=1500, valid_sets=[dva],
            callbacks=[lgb.early_stopping(80, verbose=False)],
        )
        print(f"  {name}: best_iter={booster.best_iteration}", flush=True)
        models[name] = booster
    return models


def _promo_lift(hist: pl.DataFrame, items: pl.DataFrame) -> dict:
    daily = (
        hist.join(items.select("item_nbr", "family"), on="item_nbr", how="left")
        .with_columns(pl.col("date").dt.weekday().alias("dow"))
        .group_by(["family", "store_nbr", "item_nbr", "dow", "onpromotion"])
        .agg(pl.col(TARGET).mean().alias("m"))
    )
    piv = daily.pivot(
        on="onpromotion", index=["family", "store_nbr", "item_nbr", "dow"], values="m"
    ).drop_nulls()
    cols = [c for c in piv.columns if c not in ("family", "store_nbr", "item_nbr", "dow")]
    if len(cols) != 2:
        return {"overall_pct": 0.0, "n": 0, "by_family": {}, "note": "insufficient promo contrast"}
    promo_col = next(c for c in cols if str(c).lower().startswith("t"))
    base_col = next(c for c in cols if str(c).lower().startswith("f"))
    piv = piv.with_columns(((pl.col(promo_col) + 1) / (pl.col(base_col) + 1)).alias("lift"))
    piv = piv.filter(pl.col("lift").is_finite())
    out: dict = {
        "overall_pct": round(float((piv["lift"].median() - 1) * 100), 1),
        "n": int(len(piv)),
        "by_family": {},
        "note": "median same-day-of-week ratio of mean sales on promotion vs not, per store-item; +1 smoothing.",
    }
    for fam in sorted(piv["family"].unique().to_list()):
        sub = piv.filter(pl.col("family") == fam)["lift"]
        if len(sub) >= 10:
            out["by_family"][fam] = {
                "median_pct": round(float((sub.median() - 1) * 100), 1),
                "p25_pct": round(float((sub.quantile(0.25) - 1) * 100), 1),
                "p75_pct": round(float((sub.quantile(0.75) - 1) * 100), 1),
                "n": int(len(sub)),
            }
    return out


def main() -> None:
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    (ARTIFACTS / "model").mkdir(exist_ok=True)

    hist = pl.read_parquet(INTERIM / "history.parquet")
    cal = pl.read_parquet(INTERIM / "calendar.parquet")
    items = pl.read_parquet(INTERIM / "items.parquet")
    stores = pl.read_parquet(INTERIM / "stores.parquet")

    print("building features ...", flush=True)
    df = build_features(hist, cal, items, stores)
    print(f"features: rows={len(df):,} cols={len(df.columns)}")

    print("training quantile models ...", flush=True)
    models = _train_models(df)

    # ---- backtest predictions ----------------------------------------------------
    bt = df.filter(
        (pl.col("date") >= pl.lit(BACKTEST_START).str.to_date())
        & (pl.col("date") <= pl.lit(BACKTEST_END).str.to_date())
    )
    X_bt = _as_pandas(bt)
    preds = {}
    for name, booster in models.items():
        p = np.expm1(booster.predict(X_bt, num_iteration=booster.best_iteration))
        preds[name] = np.clip(p, 0.0, None)
    # enforce quantile monotonicity row-wise
    stack = np.vstack([preds["p10"], preds["p50"], preds["p90"]])
    stack.sort(axis=0)
    p10, p50, p90 = stack[0], stack[1], stack[2]

    out = (
        bt.select("store_nbr", "item_nbr", "date", TARGET, "onpromotion")
        .with_columns(pl.Series("p10", p10), pl.Series("p50", p50), pl.Series("p90", p90))
        .join(items.select("item_nbr", "family"), on="item_nbr", how="left")
    )
    out.write_parquet(ARTIFACTS / "forecast.parquet")

    y = out[TARGET].to_numpy()
    naive7 = np.nan_to_num(bt["sales_lag_7"].to_numpy())
    naive28 = np.nan_to_num(bt["sales_mean_28"].to_numpy())

    overall = {
        "n": int(len(out)),
        "wape_p50_pct": round(_wape(y, p50) * 100, 2),
        "mae_p50": round(float(np.mean(np.abs(y - p50))), 3),
        "pinball": {
            "p10": round(_pinball(y, p10, 0.1), 4),
            "p50": round(_pinball(y, p50, 0.5), 4),
            "p90": round(_pinball(y, p90, 0.9), 4),
        },
        "coverage_p10_p90_pct": round(float(np.mean((y >= p10) & (y <= p90))) * 100, 2),
        "baseline_naive7_wape_pct": round(_wape(y, naive7) * 100, 2),
        "baseline_ma28_wape_pct": round(_wape(y, naive28) * 100, 2),
        "skill_vs_naive7_pct": round((1 - _wape(y, p50) / max(_wape(y, naive7), 1e-9)) * 100, 2),
        "bias_pct": round(float((np.sum(p50) - np.sum(y)) / max(np.sum(y), 1e-9)) * 100, 2),
    }

    by_family = {}
    for fam in sorted(out["family"].unique().to_list()):
        mask = (out["family"] == fam).to_numpy()
        if mask.sum() < 200:
            continue
        yy, yh = y[mask], p50[mask]
        by_family[fam] = {
            "n": int(mask.sum()),
            "wape_p50_pct": round(_wape(yy, yh) * 100, 2),
            "baseline_naive7_wape_pct": round(_wape(yy, naive7[mask]) * 100, 2),
            "skill_vs_naive7_pct": round((1 - _wape(yy, yh) / max(_wape(yy, naive7[mask]), 1e-9)) * 100, 2),
            "coverage_p10_p90_pct": round(float(np.mean((yy >= p10[mask]) & (yy <= p90[mask]))) * 100, 2),
            "pinball_p50": round(_pinball(yy, yh, 0.5), 4),
        }
    (ARTIFACTS / "eval_metrics.json").write_text(
        json.dumps({"overall": overall, "by_family": by_family, "window": [BACKTEST_START, BACKTEST_END]}, indent=2)
    )
    print("backtest:", json.dumps(overall, indent=2))

    # ---- attributions for the brief date (p50 booster, pred_contrib) -------------
    brief = df.filter(pl.col("date") == pl.lit(BRIEF_DATE).str.to_date())
    Xb = _as_pandas(brief)
    contrib = models["p50"].predict(Xb, num_iteration=models["p50"].best_iteration, pred_contrib=True)
    contrib = contrib[:, :-1]  # drop base value
    feat_names = NUM_FEATURES + CAT_FEATURES
    rows = []
    for i, (s, it) in enumerate(zip(brief["store_nbr"], brief["item_nbr"])):
        row = contrib[i]
        for rank, j in enumerate(np.argsort(-np.abs(row))[:5]):
            rows.append({
                "store_nbr": int(s), "item_nbr": int(it), "date": BRIEF_DATE,
                "rank": rank, "feature": feat_names[j],
                "label": FEATURE_LABELS.get(feat_names[j], feat_names[j]),
                "contrib": round(float(row[j]), 4),
            })
    pl.DataFrame(rows).write_parquet(ARTIFACTS / "attributions.parquet")
    print(f"attributions: {len(rows)} rows for {BRIEF_DATE}")

    # ---- promo lift + provenance -------------------------------------------------
    lift = _promo_lift(hist, items)
    (ARTIFACTS / "promo_lift.json").write_text(json.dumps(lift, indent=2))
    print("promo lift overall:", lift.get("overall_pct"))

    for name, booster in models.items():
        booster.save_model(str(ARTIFACTS / "model" / f"lgbm_{name}.txt"), num_iteration=booster.best_iteration)
    (ARTIFACTS / "model" / "features.json").write_text(
        json.dumps({"num": NUM_FEATURES, "cat": CAT_FEATURES, "labels": FEATURE_LABELS}, indent=2)
    )
    print("artifacts ->", ARTIFACTS)


if __name__ == "__main__":
    main()
