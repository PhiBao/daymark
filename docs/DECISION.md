# Daymark — Decision Memo & Product Specification

> AMD Developer Hackathon: ACT III (lablab.ai) · Track: **Reinvent Commerce** · Build window: 12–17 Oct 2026
> Status: approved for build (Oct 12, 2026)

---

## 1. Winning thesis

Grocery retail runs on a perishable clock. Every store, every morning, someone decides:
how much to order, what to mark down before it spoils, which promo to run, and which
deliveries arrived wrong. This work is done from gut feel and spreadsheets, and it leaks
money in two directions — **shrink** (thrown-away fresh food) and **lost sales**
(empty shelves).

Daymark is the **morning edition for perishable retail**: a decision brief published
daily that turns real transaction/inventory/weather/promo data into a short list of
actions — *order this, mark down that, promote this, check that delivery* — where
**every number is traceable to a deterministic forecaster, never to the language model**,
and an **exact-arithmetic Replay** proves the waste/margin delta against the store's
baseline policy on historical demand.

**Product wedge:** verifiable analytics narrative — *the model decides, the LLM explains,
the ledger proves.*

**Why AMD is load-bearing (not decoration):** the brief generator, promo creative, and
explanation layer are open-model inference (Gemma served via vLLM on ROCm on AMD Instinct).
This is the workload that must run **in the store back-office** — retail data cannot leave
the retailer, so a self-hosted open model on affordable AMD inference is the enabling
infrastructure. Measured: TTFT, tokens/s, $/brief on AMD Instinct.

**Why now:** EU/US food-waste regulation (UN SDG 12.3: halve retail food waste by 2030),
post-inflation margin pressure on grocery, and open multilingual models that make
per-store inference economics finally viable.

---

## 2. Hackathon intelligence (evidence)

- 5,709 participants / 1,146 teams registered; **4 submissions + 5 drafts on day 1** — huge
  attrition, competition is real but the field thins dramatically.
- Early rival submissions cluster in document/retrieval agents (DocuSwarm due-diligence,
  FiscalRAG tax, Industrial Fault Copilot manuals, 6-Gas Guardian) — **nobody is doing
  retail decision economics**.
- ACT II (1,149 projects shipped) winners: champion = VLM CCTV intelligence on MI300X
  ("no new cameras" wedge, second model verifying results); runner-up = zero-token cost
  router with exact resource accounting; third = Rust KV-cache control plane with
  MI300X-topology-aware routing and hard benchmark deltas (TTFT −36%).
- **Judge pattern:** real user + measured numbers + verification/fallbacks + crisp
  "why AMD" + honest engineering. Poster-sized claims without arithmetic lose.
- Judging criteria (equal weight): Application of Technology · Presentation ·
  Business Value · Originality. Vibe Generation special prize adds: quantified value,
  shipped execution, and **design of the human + AI workflow**.
- Evolus partner track (stretch): workspace + own vLLM-on-AMD endpoint + ≥2 Evolus
  building blocks via API. One project can win main + partner prizes.

---

## 3. Competitive whitespace

- Afresh, Shelf Engine, Wasteless, Revionics, Eversight: enterprise pricing/ordering SaaS
  for large chains — heavyweight, opaque models, no cost transparency, not self-hostable.
- Hackathon field: shopping assistants, review sentiment, churn prediction, RAG chatbots.
- **Whitespace:** a decision brief with *number-grounding* (LLM never invents a figure),
  *exact-arithmetic replay* (the product proves its own value), *per-decision cost meter*
  (genAI as a P&L line), and *self-hostable inference economics on AMD*.

---

## 4. Target user & PMF thesis

- **Primary user:** category manager / store owner-operator of a small-to-mid grocery
  chain (5–60 stores) in EU/US. Not a data scientist. Makes ordering + markdown decisions
  before 7am, in under 30 minutes, on a phone or back-office PC.
- **Job-to-be-done:** "Tell me what to do today about fresh stock so I don't throw money
  away and I don't run empty."
- **Current alternative:** spreadsheet + experience + supplier rep's advice + daily
  firefighting; enterprise tools out of reach.
- **Retention loop:** the brief is a daily ritual (habit); the ledger accumulates
  accountability; replay accumulates measured savings.
- **Distribution:** regional grocery associations, LinkedIn retail-ops communities,
  consultancies (Vibe/PwC-style), pilot with one store → chain expansion.
- **Monetization:** per-store/month SaaS (~€150–400), inference cost visible per brief.

---

## 5. Innovation (what becomes newly possible)

1. **Number-grounded generation:** every figure in a brief is emitted by the forecasting
   engine and *checked* against the ledger before display; the LLM's job is language and
   explanation only. Hallucination is structurally excluded from the numbers.
2. **Self-proving product:** Replay runs the full policy over historical demand and prints
   the exact waste/lost-sales/margin delta vs baseline — the demo *is* the evidence.
3. **Unit economics of cognition:** every generated brief/creative shows its inference
   cost (tokens × AMD price model). Judges can see that AI decisions cost cents.
4. **Human + AI workflow:** manager approves/edits; every action is signed into an audit
   ledger with its reason — designed for the Vibe Generation "hybrid team" thesis.

---

## 6. Architecture

```
                    ┌──────────────────────────────┐
   Favorita (real)  │  engine/  (Python, offline)  │
   grocery sales ──►│  curate → features → LightGBM │
   holidays, promos │  forecast + quantiles         │
   calendar, weather│  backtest WAPE/MASE per class │
   (+ simulator)    │  → artifacts: parquet, ONNX   │
                    └───────────┬──────────────────┘
                                │ artifacts (git-committed)
┌───────────────────────────────▼──────────────────────────────┐
│  backend/  (Rust, Axum)                                       │
│  ─ forecast svc (ONNX / precomputed quantiles)                │
│  ─ order policy (newsvendor on quantiles + shelf-life decay)  │
│  ─ markdown engine (age × elasticity → clear-by close)        │
│  ─ promo uplift (observed + model-based, labeled)             │
│  ─ replay engine (deterministic simulator, exact accounting)  │
│  ─ brief composer ──► vLLM OpenAI-compat ──► AMD Instinct     │
│  ─ number-grounding checker (extract → verify → regenerate)   │
│  ─ audit ledger (SQLite/sqlx) + cost meter                    │
└───────────────┬───────────────────────────────────────────────┘
                │ REST + SSE
┌───────────────▼───────────────┐
│  web/  (Next.js + TS + pnpm)  │
│  The morning edition:         │
│  Brief · Decisions · Replay   │
│  Ledger · Trust · AMD card    │
└───────────────────────────────┘
```

- **Backend language:** Rust (Axum, Tokio, Serde, SQLx, Reqwest, `ort` for ONNX).
  Python is used strictly as *offline data-science tooling* (training/backtesting) and
  produces committed artifacts — an isolated, justified exception per AGENTS.md.
- **Frontend:** Next.js (App Router), TypeScript, custom design system (no component-kit look).
- **AMD workload:** Gemma (open weights) served by vLLM on ROCm → AMD Instinct via
  AMD Developer Cloud; benchmark harness records TTFT/throughput/$ per brief.
- **Evolus (stretch):** delivery-note extractor + approval workflow driven via REST API;
  never on the critical demo path.

---

## 7. Design direction ("morning edition")

Editorial-print language: newsprint ground (#F5F2EA), ink (#141310), one produce green,
one urgency ochre; hairline rules; columnar front page; display serif (Newsreader) for
headlines, Archivo for UI, IBM Plex Mono for all figures. The brief reads like a
newspaper: lead decision, sections (Orders/Markdowns/Promotions/Deliveries), each item
= headline + number + why-line + one-click action. Replay renders as a ledger tape.
Deliberately **not** a dashboard: no widget grid, no glassmorphism, no gradient cards.
Density: calm and precise; motion: paper-talk (brief prints in, figures tick, tape runs).

---

## 8. MVP boundary

**Must ship:** curated real dataset pipeline · LightGBM forecaster + backtest metrics ·
order policy + markdown engine · deterministic replay with exact accounting vs baseline ·
brief composer on AMD-hosted Gemma with number-grounding checker · morning-edition UI
(Brief/Decisions/Replay/Ledger) · cost meter · AMD benchmark evidence · tests on core
math + API · security review · README + architecture diagram + demo video.

**Should ship:** promo uplift explainer · "why" attribution panel · weather signal ·
forecast lab (neural vs GBM honesty table) · SSE brief streaming.

**Stretch:** Evolus delivery-note extractor + workflow · Gemini layer · multi-store view.

**Non-goals:** real POS integration, auth/accounts, payments, multi-tenant SaaS, mobile
app, ASR, video generation.

---

## 9. Proof strategy (judge-visible)

- **Real data:** Corporación Favorita grocery sales (public Kaggle benchmark) — cite access + sample.
- **Real forecast metrics:** WAPE/MASE by category on held-out weeks, committed + in UI.
- **Exact replay:** deterministic simulator; policy vs baseline on identical demand;
  waste / lost-sales / service deltas reproducible from the repo (`make replay`).
  Canonical run (store 2, 2017-06-05..2017-08-13): waste 5,684 → 3,131 → 2,374 units,
  service 87.8% → 97.9% → 97.4%, −58% waste and +9.6 service points vs the spreadsheet policy.
- **Real AMD runs:** benchmark table from actual vLLM-ROCm runs (model, GPU, quant, tokens/s,
  TTFT, $/brief) + in-app AMD card + raw logs committed.
- **Honesty:** assumptions page (censored demand, simulator calibration, modeled promo
  uplift) — one business result, one limitation, one failure case, safeguards, per the brief.

---

## 10. Remaining risks

| Risk | Mitigation |
|---|---|
| Kaggle/Favorita access friction | mirror via HuggingFace Hub; fallback Rossmann/UCI + calibrated simulator, disclosed |
| AMD cloud setup time / quota | scripts + instructions prepared day 0; dev against local OpenAI-compat server; AMD endpoint for demo + benchmarks |
| Perishable shelf-life absent in dataset | category-level shelf-life table, disclosed as assumption; simulator holds ground truth |
| Replay credibility ("it's a simulation") | exact arithmetic + disclosed assumptions + real-data forecast validation + sensitivity section |
| Scope creep | stretch/non-goals above are frozen |
| 5-day deadline | day-ordered plan; core proof loop first, polish after |
