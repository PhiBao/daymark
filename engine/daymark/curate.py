"""Curate the Favorita dataset into Daymark's demo chain.

Outputs (data/interim):
  history.parquet    store_nbr, item_nbr, date, unit_sales, onpromotion
  calendar.parquet   date, store_nbr, dow, is_weekend, holiday flags, transactions, oil_usd
  items.parquet      item_nbr, family, class, perishable, shelf_life_days, unit_price_usd
  stores.parquet     store_nbr, city, state, type, cluster

Design notes
------------
* We curate 3 stores with full history and the 9 fresh families (see config).
* Negative unit_sales (returns) are clipped to 0.
* Item/day combinations absent from the source are zero-filled for sales
  (no sale recorded that day) so lag/rolling features are well-defined.
* Assumptions (shelf life, unit price) are injected here and surfaced in
  artifacts/meta.json — the API/UI display them next to every derived number.
"""

from __future__ import annotations

import json

import polars as pl

from daymark.config import (
    FRESH_FAMILIES,
    INTERIM,
    RAW,
    SHELF_LIFE_DAYS,
    UNIT_PRICE_USD,
)

N_STORES = 3
TOP_ITEMS_PER_FAMILY = 30


def _pick_stores(scan: pl.LazyFrame, stores: pl.DataFrame) -> list[int]:
    """Pick stores with full daily coverage; prefer cluster/type diversity."""
    coverage = (
        scan.group_by("store_nbr")
        .agg(pl.col("date").n_unique().alias("days"))
        .collect(engine="streaming")
    )
    joint = coverage.join(stores, on="store_nbr", how="inner").sort(
        ["days", "store_nbr"], descending=[True, False]
    )
    top = joint.head(20)
    picked: list[dict] = []
    seen_clusters: set[int] = set()
    for row in top.iter_rows(named=True):
        if row["cluster"] in seen_clusters:
            continue
        seen_clusters.add(row["cluster"])
        picked.append(row)
        if len(picked) == N_STORES:
            break
    # widen if clusters too few
    i = 0
    while len(picked) < N_STORES and i < len(top):
        row = top.row(i, named=True)
        if row["store_nbr"] not in {p["store_nbr"] for p in picked}:
            picked.append(row)
        i += 1
    return sorted(int(p["store_nbr"]) for p in picked)


def main() -> None:
    INTERIM.mkdir(parents=True, exist_ok=True)
    print("scanning train.parquet (this reads all row groups once) ...", flush=True)

    lf = pl.scan_parquet(RAW / "train.parquet")
    items = pl.read_parquet(RAW / "items.parquet")
    stores = pl.read_parquet(RAW / "stores.parquet")
    holidays = pl.read_parquet(RAW / "holidays_events.parquet")
    oil = pl.read_csv(RAW / "oil.csv", try_parse_dates=True).rename({"dcoilwtico": "dcoilwtico"})
    transactions = pl.read_parquet(RAW / "transactions.parquet")
    print(f"items={len(items):,} stores={len(stores)} holidays={len(holidays):,}")

    fresh_items = items.filter(pl.col("family").is_in(FRESH_FAMILIES))
    picked_stores = _pick_stores(lf, stores)
    print("picked stores:", picked_stores)

    hist = (
        lf.filter(
            pl.col("store_nbr").is_in(picked_stores)
            & pl.col("item_nbr").is_in(fresh_items["item_nbr"].implode())
        )
        .select(
            pl.col("store_nbr").cast(pl.Int16),
            pl.col("item_nbr").cast(pl.Int32),
            pl.col("date").str.to_date(),
            pl.col("unit_sales").cast(pl.Float32),
            pl.col("onpromotion").cast(pl.Boolean, strict=False).fill_null(False),
        )
        .group_by(["store_nbr", "item_nbr", "date"])
        .agg(
            pl.col("unit_sales").sum(),
            pl.col("onpromotion").max(),
        )
        .with_columns(pl.col("unit_sales").clip(lower_bound=0))
        .collect(engine="streaming")
        .sort(["store_nbr", "item_nbr", "date"])
    )
    print(f"history rows={len(hist):,} span={hist['date'].min()}..{hist['date'].max()}")

    # ---- zero-fill item/day grid -------------------------------------------------
    grid = (
        hist.select("store_nbr", "item_nbr")
        .unique()
        .join(
            pl.DataFrame(
                {"date": pl.date_range(hist["date"].min(), hist["date"].max(), "1d", eager=True)}
            ),
            how="cross",
        )
    )
    hist = (
        grid.join(hist, on=["store_nbr", "item_nbr", "date"], how="left")
        .with_columns(
            pl.col("unit_sales").fill_null(0.0),
            pl.col("onpromotion").fill_null(False),
        )
        .sort(["store_nbr", "item_nbr", "date"])
    )

    # ---- calendar ---------------------------------------------------------------
    nat = holidays.filter(pl.col("locale") == "National")
    nat_types = (
        nat.select(
            pl.col("date").str.to_date(),
            pl.col("type").alias("holiday_type"),
            pl.col("description").alias("holiday_name"),
            pl.col("transferred"),
        )
        .sort("date")
        .unique(subset=["date"], keep="first")
    )
    loc = holidays.filter(pl.col("locale") == "Local").select(
        pl.col("date").str.to_date(),
        pl.col("locale_name").alias("city"),
        pl.col("description").alias("local_holiday_name"),
    )
    oil_ff = (
        oil.sort("date")
        .select(pl.col("date"), pl.col("dcoilwtico").cast(pl.Float32))
        .with_columns(pl.col("dcoilwtico").forward_fill())
    )
    dates = pl.DataFrame(
        {"date": pl.date_range(hist["date"].min(), hist["date"].max(), "1d", eager=True)}
    )
    store_meta = stores.filter(pl.col("store_nbr").is_in(picked_stores)).select(
        "store_nbr", "city", "state", "type", "cluster"
    )
    cal = (
        dates.join(store_meta, how="cross")
        .join(transactions.with_columns(pl.col("date").str.to_date()), on=["store_nbr", "date"], how="left")
        .join(nat_types, on="date", how="left")
        .join(loc, on=["date", "city"], how="left")
        .join(oil_ff, on="date", how="left")
        .with_columns(
            pl.col("date").dt.weekday().alias("dow"),
            pl.col("date").dt.month().alias("month"),
            pl.col("date").dt.day().alias("dom"),
            pl.col("date").dt.week().alias("woy"),
            pl.col("transactions").fill_null(strategy="forward"),
            pl.col("dcoilwtico").fill_null(strategy="forward").backward_fill(),
        )
        .with_columns(
            (pl.col("dow") >= 6).alias("is_weekend"),
            pl.col("holiday_type").is_not_null().alias("is_holiday"),
            pl.col("local_holiday_name").is_not_null().alias("is_local_holiday"),
            (pl.col("holiday_name") == "Dia de la Madre").alias("is_mothers_day"),
            ((pl.col("dom") == 15) | (pl.col("dom") >= 28) | (pl.col("dom") <= 2)).alias("is_payday"),
        )
        .select(
            "store_nbr", "date", "dow", "is_weekend", "month", "dom", "woy",
            "is_holiday", "is_local_holiday", "is_mothers_day", "is_payday",
            "holiday_name", "transactions", "dcoilwtico",
        )
        .sort(["store_nbr", "date"])
    )

    # ---- keep the demo assortment: top items by chain volume per family ----------
    top_items = (
        hist.group_by(["item_nbr"])
        .agg(pl.col("unit_sales").sum().alias("total"))
        .join(items.filter(pl.col("family").is_in(FRESH_FAMILIES)).select("item_nbr", "family"), on="item_nbr", how="inner")
        .sort(["family", "total"], descending=[False, True])
        .group_by("family")
        .head(TOP_ITEMS_PER_FAMILY)
        .select("item_nbr")
    )
    hist = hist.filter(pl.col("item_nbr").is_in(top_items["item_nbr"].implode()))
    print(f"assortment: {hist['item_nbr'].n_unique()} items/family-set, rows={len(hist):,}")

    # ---- item / store metadata + assumptions -------------------------------------
    items_out = (
        fresh_items.select(
            pl.col("item_nbr").cast(pl.Int32),
            pl.col("family"),
            pl.col("class").cast(pl.Int16),
            pl.col("perishable").cast(pl.Boolean),
        )
        .with_columns(
            pl.col("family").replace_strict(SHELF_LIFE_DAYS, default=4, return_dtype=pl.Int16).alias("shelf_life_days"),
            pl.col("family").replace_strict(UNIT_PRICE_USD, default=2.5, return_dtype=pl.Float32).alias("unit_price_usd"),
        )
    )
    items_used = pl.DataFrame(
        {"item_nbr": hist["item_nbr"].unique()}
    ).join(items_out, on="item_nbr", how="left")
    stores_out = store_meta.with_columns(
        pl.col("store_nbr").cast(pl.Int16),
        pl.col("cluster").cast(pl.Int16),
    ).sort("store_nbr")

    hist.write_parquet(INTERIM / "history.parquet")
    cal.write_parquet(INTERIM / "calendar.parquet")
    items_used.write_parquet(INTERIM / "items.parquet")
    stores_out.write_parquet(INTERIM / "stores.parquet")

    meta = {
        "source": "Corporacion Favorita grocery sales (Kaggle benchmark), mirrored on HuggingFace as hsnalmasri/favorita",
        "source_url": "https://huggingface.co/datasets/hsnalmasri/favorita",
        "stores": [int(s) for s in picked_stores],
        "families": sorted(hist.join(items_used, on="item_nbr", how="left")["family"].unique().to_list()),
        "items": int(hist["item_nbr"].n_unique()),
        "rows": int(len(hist)),
        "date_min": str(hist["date"].min()),
        "date_max": str(hist["date"].max()),
        "assumptions": {
            "shelf_life_days_by_family": SHELF_LIFE_DAYS,
            "unit_price_usd_by_family": UNIT_PRICE_USD,
            "note": "Favorita ships no inventory, expiry or price data. Shelf life and prices are our declared assumptions; unit-based metrics do not depend on prices.",
        },
    }
    (INTERIM / "meta.json").write_text(json.dumps(meta, indent=2))
    print("curated ->", INTERIM)


if __name__ == "__main__":
    main()
