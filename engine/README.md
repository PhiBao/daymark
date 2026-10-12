# Daymark engine (offline)

Python data-science tooling — the only Python in the project, by design: the
forecasting model is trained and validated here, and every artifact it produces
is committed so the Rust backend (the product core) stays the only runtime.

```bash
uv sync
uv run python -m daymark.fetch            # source data (~900MB, once)
uv run python -m daymark.curate           # 3 stores × 242 fresh SKUs, features-ready
uv run python -m daymark.train            # LightGBM quantiles + backtest + attributions
uv run python -m daymark.export_serving   # serving bundle for the Rust API
uv run python -m daymark.bench            # AMD endpoint benchmark (see infra/amd)
```

Outputs land in `artifacts/` (committed contract — see `backend/src/artifacts.rs`):
forecast bands, eval metrics, attributions ("why" chips), measured promo lift,
calendar, history window, model provenance.

Key numbers (out-of-sample, held-out weeks): WAPE 26.7% · skill +27.1% vs
same-weekday · band coverage 78.8% · bias −1.5%.
