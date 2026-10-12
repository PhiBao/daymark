"""Export the serving bundle the Rust backend reads.

Reads curated interim data + trained artifacts and writes everything the API
needs into engine/artifacts/ with a stable contract (see backend/src/artifacts.rs).
"""

from __future__ import annotations

import shutil

import polars as pl

from daymark.config import ARTIFACTS, INTERIM

HISTORY_WINDOW_START = "2017-05-01"


def main() -> None:
    hist = pl.read_parquet(INTERIM / "history.parquet").filter(
        pl.col("date") >= pl.lit(HISTORY_WINDOW_START).str.to_date()
    )
    hist.write_parquet(ARTIFACTS / "history_window.parquet")
    shutil.copy(INTERIM / "calendar.parquet", ARTIFACTS / "calendar.parquet")
    shutil.copy(INTERIM / "items.parquet", ARTIFACTS / "items.parquet")
    shutil.copy(INTERIM / "stores.parquet", ARTIFACTS / "stores.parquet")
    shutil.copy(INTERIM / "meta.json", ARTIFACTS / "meta.json")
    print(f"history_window: {len(hist):,} rows, {hist['date'].min()}..{hist['date'].max()}")


if __name__ == "__main__":
    main()
