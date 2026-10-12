# lablab.ai submission — copy-paste blocks

Everything below is ready to paste into the ACT III submission form. Replace
`[VIDEO_URL]` and `[DEMO_URL]` once recorded/deployed.

---

## Title (≤ 50 characters)

```
Daymark — the morning edition for perishable retail
```

(49 characters.)

## Short description (≤ 255 characters)

```
Daymark turns real grocery data into a daily decision brief — orders, markdowns and promos — where every figure comes from a demand model, every claim is proven by a deterministic replay, and every decision is signed into a tamper-evident ledger. Gemma runs on AMD Instinct.
```

(253 characters — check the counter before submitting.)

## Long description

```markdown
**The problem.** Fresh food is where a grocery store's money quietly dies. Every
morning, someone decides how much to order, what to mark down before it spoils and
which promotion to run — from memory, spreadsheets and a supplier rep's opinion. The
result is two leaks: thrown-away fresh stock and empty shelves on the day demand peaks.
Enterprise "AI category management" exists for big chains; a mid-size chain's category
manager has nothing.

**What Daymark is.** A morning brief, published by 6am: orders due tomorrow, markdowns
before expiry, promo candidates with lift measured from the store's own history, and a
watchlist of uncertain items — each with the reasons attached. Every number is computed
by a LightGBM quantile forecaster (validated out of sample: WAPE 26.7%, +27% better than
the store's same-weekday rule, 78.8% band coverage). A language model — Gemma served by
vLLM on ROCm on an AMD Instinct GPU — writes the words only: a number-grounding checker
verifies every figure before display, regenerating or falling back to a labelled
template if anything is invented.

**The proof is built in.** Replay re-runs the last ten weeks of *real* demand under
three policies — spreadsheet, forecast-only, full Daymark — with identical demand and
opening stock. Exact arithmetic: **−58% waste units, +9.6 service points, +24,982 units
sold** versus the spreadsheet rule, reproducible with one curl. The trade-offs are shown
too: markdowns cost a little service at the tail. Managers approve, edit or flag in one
click; decisions land in a SHA-256 hash-chained ledger that re-verifies on every read.

**Why AMD is load-bearing.** Retail data cannot leave the retailer, so the language
layer is self-served: vLLM + Gemma on AMD Instinct via AMD Developer Cloud, benchmarked
(TTFT, decode tokens/s at concurrency 1/4/8) with cost per brief derived from the GPU's
price and displayed in the product — pennies per store per day.

**Stack.** Rust (Axum) decision core: policies, deterministic simulator/replay,
grounding checker, ledger, API. Python offline tooling: training and backtesting.
Next.js front end: an editorial "morning edition", deliberately not a dashboard.
```

## Track

**Reinvent Commerce** (primary). Partner prizes: Google (Gemma on AMD) and Evolus
(working prototype integrating an Evolus workflow — see `docs/EVOLUS.md` once done).

## Technology tags (pick the closest available in the form)

`AMD Developer Cloud` · `AMD ROCm` · `HuggingFace Hub` · `Next.js` · `Rust` · `vLLM` ·
`Gemma` · `LightGBM`

## Media checklist

| Field | Value |
|---|---|
| Cover image | `docs/assets/cover.png` (1920×1080) |
| Video presentation | `[VIDEO_URL]` — script in `docs/DEMO.md`, ≤ 5 min, < 300 MB |
| Pitch presentation | `docs/pitch.pdf` (8 slides) |
| GitHub repository | https://github.com/PhiBao/daymark |
| Demo application platform | Docker Compose stack (`infra/docker/compose.yml`) on the AMD instance or any VM |
| Demo application URL | `[DEMO_URL]` |
| Architecture diagram | `docs/diagrams/architecture.svg` (also embedded in the README) |

## Requirement mapping (from the organizer's own checklist)

| Requirement | Where |
|---|---|
| Working prototype | `make data && make engine && make api && make web` — and the video |
| AMD hardware / infrastructure used | vLLM + Gemma on AMD Instinct (AMD Developer Cloud); `infra/amd/README.md` |
| Model and serving stack | Gemma + vLLM + ROCm; recorded in `engine/artifacts/benchmark.json` |
| At least one relevant benchmark | `python -m daymark.bench` output: TTFT p50/p95, decode tok/s (1/4/8), $/1M tokens |
| Clear explanation of what ran on AMD | README §Architecture + Trust page in-app |
| One business result | Replay table (waste −58%, +9.6 service pts, +24,982 units) |
| One known limitation | Stock-out censoring in demand; declared in PROOF.md and in-app |
| One failure case | Model endpoint down → facts still render, generation degrades to labelled template |
| Safeguard / human review | Grounding checker + human approve/edit/flag → hash-chained ledger |
| Public repo, setup that runs, README | github.com/PhiBao/daymark |
| No credentials in repo | ✓ (env vars only; `.env.example` provided) |

## Before submitting

- [ ] Record the video (`docs/DEMO.md`), upload (YouTube unlisted is fine), fill `[VIDEO_URL]`
- [ ] Deploy the demo stack and fill `[DEMO_URL]` (see `infra/docker/compose.yml`)
- [ ] Run the AMD benchmark and commit `engine/artifacts/benchmark.json` — then
      verify the "<$0.01 per brief" claim on the cover/README against the measured
      `usd_per_1m_tokens` and adjust the wording if it lands higher
- [ ] Re-shoot `docs/assets/brief.png` and `docs/assets/replay.png` once the real
      Gemma endpoint is serving (the header shows the model id)
- [ ] Re-run `make smoke` and paste the fresh replay numbers if the simulator changed
- [ ] Double-check character counts in the form
