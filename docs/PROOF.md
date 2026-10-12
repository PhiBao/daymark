# What we promise the judge, and where the receipt is

Daymark's presentation makes specific claims. Every one of them is backed by
something a judge can inspect. Mocked or simulated behaviour is labelled.

| Claim | Where it is proven | Real / simulated |
|---|---|---|
| Demand forecasting works on real data | `engine/artifacts/eval_metrics.json`, `/api/trust`, Trust page — WAPE, skill vs same-weekday baseline, band coverage, bias on held-out weeks | **Real** — out-of-sample on 54,872 item-days of Corporación Favorita (public benchmark) |
| The language layer never invents a figure | `backend/src/grounding.rs` (checker + tests), `narrative.grounding` in every brief, `source: model / model_retry / template_fallback` | **Real** — violations trigger one regeneration, then a deterministic template |
| The model runs on AMD | `engine/artifacts/benchmark.json`, `/api/benchmark`, Trust page; `infra/amd/` scripts | **Real** — recorded from a vLLM-ROCm endpoint on AMD Developer Cloud via `python -m daymark.bench`. Until the run is recorded, the API reports the cost model as `estimated` and the Trust page says so — it never pretends. |
| Replay proves waste/margin deltas | `/api/replay`, Replay page, `bash scripts/smoke.sh` | **Simulated, disclosed** — a deterministic perishable-store simulator; demand = recorded sales; shelf life and unit prices are declared assumptions; identical demand/opening stock for every policy |
| Decisions are tamper-evident | `/api/ledger` (hash chain re-verified on read), `backend/src/ledger.rs` tests | **Real** — SQLite + SHA-256 chain |
| The product costs pennies to run | `/api/trust` cost model, per-brief `cost_usd` | **Derived** — GPU-hour price ÷ measured throughput (disclosed formula) |

## Known limitations (stated, not hidden)

- Demand = recorded sales; stock-out censoring is acknowledged.
- Shelf life and unit prices are assumed per category (dataset has none).
- Promotion lift is measured from history; the markdown engine caps policy lift
  at 40% conservatively while showing the raw measurement.
- Markdowns increase effective demand (customers respond) which can raise
  "lost sales" at the tail — the replay shows this trade-off rather than hiding it.
- Display aliases for products are "Category · item <id>": the source dataset
  anonymizes items.
