# Demo video script (≤ 5:00, lablab limit)

Record at 1920×1080. Keep the browser at 100% zoom, window ≥ 1440px wide so the
brief shows its two-column layout. Pre-warm: `make api` + `make web`, open the
app once and press **Print today's brief** so the narrative is cached (instant on
camera), then **reload** to the unprinted state for the take.

Fallbacks are marked ⟳. Total runtime target: 4:40.

---

## 0:00–0:25 — Hook (talking head over the brief)

> "Every supermarket throws away fresh food and still runs out of it. Both
> losses come from the same thirty minutes: the decisions someone makes before
> seven a.m. This is Daymark — the morning edition for perishable retail."

Show: cursor over the sell-through of the front page, slowly.

## 0:25–0:50 — Who it's for

> "Meet the category manager of a mid-size grocery chain. Not a data scientist.
> Today she has a spreadsheet, her memory, and a supplier rep. At six a.m. she
> gets this."

Click **Print today's brief**. Watch the edition set (⟳ if slow: it's cached,
it should be instant).

> "One page: what to order for tomorrow, what to mark down before it expires,
> what to promote, and which items the model is least sure about."

## 0:50–1:30 — The numbers are not written by AI

Scroll to the orders section. Hover a why-chip line (▲ 7-day average).

> "Every figure here was computed by a demand model — LightGBM quantiles,
> validated out of sample: twenty-seven percent better than the store's own
> same-weekday rule, with honest uncertainty bands."

Point at the *figures verified* badge under the headline.

> "The language model — Gemma, running on an AMD Instinct GPU — only writes the
> words. Every number it writes is checked against the facts before you see it.
> Seventeen figures verified, zero invented. If it ever failed, you'd see it
> here — it can't hallucinate a number into a business decision."

## 1:30–2:05 — Human in the loop, on the record

Approve one order (click **Approve**). Then a markdown line (**Mark down**).

> "She approves, edits or flags with one click. Every decision lands in the
> ledger — signed with the hash of the entry before it."

Click **Ledger** in the nav. Point at *chain verified*, show two rows.

> "Change any historical row afterwards and the chain breaks. For a business
> putting AI into ordering, that's the difference between a tool and a liability."

## 2:05–3:20 — The proof: Replay (the wow)

Click **Replay**.

> "Here's what I like most: the product proves its own value. Same store, same
> ten weeks of real demand, same opening shelves — run three ways: the
> spreadsheet rule, forecasts alone, and the full Daymark policy."

Drag the **Markdown lift ×** dial to 0.5 and back while numbers recompute.

> "That's not a promise in a pitch deck — it's exact arithmetic you can re-run
> with one curl. Fifty-eight percent less waste than the spreadsheet, nine and a half
> service points better, twenty-five thousand more units sold. And notice: the
> trade-offs are shown too — markdowns cost a bit of service at the tail, and
> lift units at a discount. We don't hide the cost of our own advice."

Point at the waterfall chart's annotation lines, then at *Reproduce this exact
replay* (open the details once).

## 3:20–4:00 — Why AMD is load-bearing

Click **Trust**.

> "Two things make this economically real. First, the model runs on our own
> infrastructure: Gemma served by vLLM on ROCm, on an AMD Instinct GPU — no data
> leaves the retailer. Second, every brief carries its own cost."

Point at the cost model line and the benchmark numbers.

> "Measured: TTFT and decode throughput at concurrency one through eight, and
> dollars per million tokens from the GPU's hourly price. A brief costs a
> fraction of a cent — that's why this can exist per store, not just per chain."

## 4:00–4:35 — Why it matters, why now

Back to the **Brief**. Scroll through markdowns and the watchlist.

> "Food waste is a margin problem and a climate problem — the EU wastes 153
> million tonnes a year, and the UN target is to halve retail waste by 2030.
> Mid-size chains can't buy an enterprise AI pricing suite. This is the same
> category of decision, self-hosted, auditable, and cheap enough for every store."

## 4:35–4:55 — Close

> "Daymark: the model decides, the LLM explains, the ledger proves. Built for
> AMD's ACT III hackathon, Reinvent Commerce track. Everything you saw is in the
> public repo — including the numbers."

End card (2s): wordmark + repo URL + "Data: Corporación Favorita public benchmark".

---

## Shot checklist

1. Talking head / cursor over front page
2. Edition setting animation + verified badge
3. Why-chips + interval bars on one order row
4. Approve → markdown → Ledger (chain verified, hash column)
5. Replay: deltas strip → dial drag → by-family table → reproduce curl
6. Trust: forecast table → AMD benchmark → cost model
7. Back to Brief: markdowns + watchlist
8. End card

## If something breaks on camera

- Model endpoint down → the brief still renders all figures; the badge will say
  *template copy*; say honestly: "the language layer is offline here; the facts
  and the proof don't depend on it."
- Replay slow → it is pure local Rust; if the first run lags, re-run; the second
  is cached in memory.
- Keep the *Reproduce this exact replay* curl visible at least 3 seconds — it is
  the strongest single proof frame in the video.
