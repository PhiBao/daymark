"""Daymark engine config: paths, constants, assumptions.

All assumptions that touch credibility are centralized here and surfaced in
artifacts/meta.json so the API/UI/README can display them next to the numbers.
"""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RAW = ROOT / "data" / "raw" / "favorita"
INTERIM = ROOT / "data" / "interim"
ARTIFACTS = ROOT / "engine" / "artifacts"

# Corporación Favorita (Ecuador) grocery sales — public Kaggle benchmark,
# mirrored on HuggingFace Hub (`hsnalmasri/favorita`; oil prices come from the
# `khadidia-77/favorita` mirror which keeps the original CSV).
HF_DATASET = "hsnalmasri/favorita"
HF_OIL = ("khadidia-77/favorita", "oil.csv")
RAW_FILES = [
    "train.parquet",
    "items.parquet",
    "stores.parquet",
    "holidays_events.parquet",
    "transactions.parquet",
]

# Fresh/perishable families we curate for the demo chain.
FRESH_FAMILIES = [
    "PRODUCE",
    "BREAD/BAKERY",
    "DAIRY",
    "MEATS",
    "SEAFOOD",
    "DELI",
    "POULTRY",
    "EGGS",
    "PREPARED FOODS",
]

# Shelf-life (days) by family — ASSUMPTION (dataset has no expiry field).
# Sourced from typical retail fresh categories; sensitivity-tested in replay.
SHELF_LIFE_DAYS = {
    "PRODUCE": 5,
    "BREAD/BAKERY": 3,
    "DAIRY": 10,
    "MEATS": 4,
    "SEAFOOD": 2,
    "DELI": 4,
    "POULTRY": 3,
    "EGGS": 21,
    "PREPARED FOODS": 3,
}

# Average unit price per family (USD) — ASSUMPTION for €-value reporting only.
# Unit-based metrics (waste units, service level) do not depend on this.
UNIT_PRICE_USD = {
    "PRODUCE": 1.20,
    "BREAD/BAKERY": 1.60,
    "DAIRY": 2.40,
    "MEATS": 4.80,
    "SEAFOOD": 6.00,
    "DELI": 3.20,
    "POULTRY": 3.60,
    "EGGS": 2.20,
    "PREPARED FOODS": 3.40,
}

# Markdown price cut considered by the policy (fraction off).
MARKDOWN_CUTS = [0.30, 0.50]

# Service-level target used by the newsvendor order policy (fresh).
SERVICE_LEVEL = 0.92

# Data windows (Favorita ends 2017-08-15).
TRAIN_END = "2017-05-31"
BACKTEST_START = "2017-06-01"
BACKTEST_END = "2017-08-15"
# The "today" the demo brief is published for.
BRIEF_DATE = "2017-08-14"  # Monday
REPLAY_START = "2017-06-05"  # Monday
REPLAY_END = "2017-08-13"  # Sunday
