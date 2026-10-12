"""Feature engineering for the Daymark demand forecaster.

Single source of truth for features so training, backtesting and any future
online inference cannot drift apart.
"""

from __future__ import annotations

import polars as pl

GROUP = ["store_nbr", "item_nbr"]

STATIC_CAT_FEATURES = ["family", "class", "store_type", "cluster", "store_nbr", "item_nbr"]
NUM_FEATURES = [
    "dow", "is_weekend", "month", "dom", "woy",
    "is_holiday", "is_local_holiday", "is_mothers_day", "is_payday",
    "onpromotion",
    "sales_lag_1", "sales_lag_7", "sales_lag_14", "sales_lag_28",
    "sales_mean_7", "sales_mean_14", "sales_mean_28",
    "sales_std_7", "sales_std_28",
    "sales_median_28",
    "promo_count_7",
    "transactions", "transactions_mean_7",
    "dcoilwtico",
]
CAT_FEATURES = ["family", "class", "store_type", "cluster", "store_nbr", "item_nbr"]
TARGET = "unit_sales"


def build_features(
    hist: pl.DataFrame,
    cal: pl.DataFrame,
    items: pl.DataFrame,
    stores: pl.DataFrame,
) -> pl.DataFrame:
    df = (
        hist.join(cal, on=["store_nbr", "date"], how="left")
        .join(items, on="item_nbr", how="left")
        .join(
            stores.select(
                "store_nbr",
                pl.col("type").alias("store_type"),
                pl.col("cluster").cast(pl.Int16),
            ),
            on="store_nbr",
            how="left",
        )
        .sort(GROUP + ["date"])
    )

    g = GROUP
    # Yesterday-close semantics: every feature is known before date t.
    df = df.with_columns(
        [
            pl.col(TARGET).shift(1).over(g).alias("_s1"),
            pl.col("onpromotion").cast(pl.Int8).shift(1).over(g).alias("_p1"),
            pl.col("transactions").shift(1).over(g).alias("_t1"),
        ]
    )
    df = df.with_columns(
        [pl.col(TARGET).shift(k).over(g).alias(f"sales_lag_{k}") for k in (1, 7, 14, 28)]
    )
    df = df.with_columns(
        [
            pl.col("_s1").rolling_mean(7, min_samples=1).over(g).alias("sales_mean_7"),
            pl.col("_s1").rolling_mean(14, min_samples=1).over(g).alias("sales_mean_14"),
            pl.col("_s1").rolling_mean(28, min_samples=1).over(g).alias("sales_mean_28"),
            pl.col("_s1").rolling_std(7, min_samples=2).over(g).alias("sales_std_7"),
            pl.col("_s1").rolling_std(28, min_samples=2).over(g).alias("sales_std_28"),
            pl.col("_s1").rolling_median(28, min_samples=1).over(g).alias("sales_median_28"),
            pl.col("_p1").rolling_sum(7, min_samples=1).over(g).alias("promo_count_7"),
            pl.col("_t1").rolling_mean(7, min_samples=1).over(g).alias("transactions_mean_7"),
        ]
    ).drop(["_s1", "_p1", "_t1"])
    df = df.with_columns(
        [
            pl.col("store_nbr").cast(pl.Int16),
            pl.col("item_nbr").cast(pl.Int32),
            pl.col("class").cast(pl.Int16),
            pl.col("cluster").cast(pl.Int16),
            pl.col("family").cast(pl.Categorical),
            pl.col("store_type").cast(pl.Categorical),
            pl.col("dow").cast(pl.Int8),
            pl.col("month").cast(pl.Int8),
            pl.col("dom").cast(pl.Int8),
            pl.col("woy").cast(pl.Int8),
            pl.col("onpromotion").cast(pl.Int8),
            pl.col("is_weekend").cast(pl.Int8),
            pl.col("is_holiday").cast(pl.Int8),
            pl.col("is_local_holiday").cast(pl.Int8),
            pl.col("is_mothers_day").cast(pl.Int8),
            pl.col("is_payday").cast(pl.Int8),
        ]
    )
    keep: list[str] = []
    for c in ["store_nbr", "item_nbr", "date", TARGET, "onpromotion"] + NUM_FEATURES + STATIC_CAT_FEATURES:
        if c not in keep:
            keep.append(c)
    return df.select(keep)
