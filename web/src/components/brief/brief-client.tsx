"use client";

import { useState } from "react";
import Link from "next/link";
import {
  API_URL,
  BriefFacts,
  BriefResponse,
  MarkdownLine,
  Narrative,
  OrderLine,
  PromoLine,
  WhyChip,
  fmt,
  postJSON,
} from "@/lib/api";

type DecisionState = Record<number, { action: string; hash: string }>;

function Chip({ children, tone = "ink" }: { children: React.ReactNode; tone?: "ink" | "green" | "ochre" | "red" | "muted" }) {
  const tones: Record<string, string> = {
    ink: "border-[var(--color-ink)] text-[var(--color-ink)]",
    green: "border-[var(--color-green)] text-[var(--color-green)]",
    ochre: "border-[var(--color-ochre)] text-[var(--color-ochre)]",
    red: "border-[var(--color-red)] text-[var(--color-red)]",
    muted: "border-[var(--color-rule)] text-[var(--color-muted)]",
  };
  return (
    <span className={`inline-flex items-center gap-1 border px-1.5 py-0.5 text-[10px] tracking-[0.08em] uppercase ${tones[tone]}`}>
      {children}
    </span>
  );
}

function Why({ chips, max = 3 }: { chips: WhyChip[]; max?: number }) {
  if (!chips.length) return null;
  return (
    <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1">
      {chips.slice(0, max).map((c) => (
        <span key={c.feature} className="inline-flex items-center gap-1 text-[11px] text-[var(--color-muted)]">
          <span
            aria-hidden
            className={c.contrib >= 0 ? "text-[var(--color-green)]" : "text-[var(--color-red)]"}
          >
            {c.contrib >= 0 ? "▲" : "▼"}
          </span>
          {c.label}
        </span>
      ))}
    </div>
  );
}

function Interval({ p10, p50, p90, scale }: { p10: number; p50: number; p90: number; scale: number }) {
  const w = 96;
  const x = (v: number) => Math.max(0, Math.min(w, (v / scale) * w));
  return (
    <svg width={w} height={14} className="overflow-visible" role="img" aria-label={`forecast ${p50} (${p10} to ${p90})`}>
      <line x1={x(p10)} x2={x(p90)} y1={7} y2={7} stroke="var(--color-rule)" strokeWidth={3} strokeLinecap="round" />
      <circle cx={x(p50)} cy={7} r={3.2} fill="var(--color-ink)" />
    </svg>
  );
}

function SectionHeading({ kicker, title, note }: { kicker: string; title: string; note?: string }) {
  return (
    <div className="rule-top mb-3 pt-3">
      <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">{kicker}</div>
      <h2 className="display text-[1.35rem] leading-tight font-semibold">{title}</h2>
      {note ? <p className="mt-0.5 text-[12.5px] text-[var(--color-muted)]">{note}</p> : null}
    </div>
  );
}

function ActionButton({
  label,
  onClick,
  tone = "ink",
}: {
  label: string;
  onClick: () => void;
  tone?: "ink" | "green" | "red" | "muted";
}) {
  const tones: Record<string, string> = {
    ink: "border-[var(--color-ink)] hover:bg-[var(--color-ink)] hover:text-[var(--color-paper)]",
    green: "border-[var(--color-green)] text-[var(--color-green)] hover:bg-[var(--color-green)] hover:text-[var(--color-paper)]",
    red: "border-[var(--color-red)] text-[var(--color-red)] hover:bg-[var(--color-red)] hover:text-[var(--color-paper)]",
    muted: "border-[var(--color-rule)] text-[var(--color-muted)] hover:border-[var(--color-ink)] hover:text-[var(--color-ink)]",
  };
  return (
    <button
      type="button"
      onClick={onClick}
      className={`border px-2 py-0.5 text-[10.5px] tracking-[0.08em] uppercase transition-colors ${tones[tone]}`}
    >
      {label}
    </button>
  );
}

export default function BriefClient({
  store,
  date,
  initialFacts,
  model,
}: {
  store: number;
  date: string;
  initialFacts: BriefFacts | null;
  model: string;
}) {
  const [facts] = useState<BriefFacts | null>(initialFacts);
  const [brief, setBrief] = useState<BriefResponse | null>(null);
  const [printing, setPrinting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [decisions, setDecisions] = useState<DecisionState>({});
  const [editItem, setEditItem] = useState<number | null>(null);
  const [editQty, setEditQty] = useState<string>("");
  const [showAllOrders, setShowAllOrders] = useState(false);
  const [showAllMarkdowns, setShowAllMarkdowns] = useState(false);

  if (!facts) {
    return (
      <div className="rule-top mt-8 border border-[var(--color-rule)] bg-[var(--color-paper-2)] p-8 text-center">
        <div className="display text-2xl">The press is cold.</div>
        <p className="mx-auto mt-2 max-w-[46ch] text-[13.5px] text-[var(--color-muted)]">
          The Daymark API is not reachable from the web app. Start it with{" "}
          <code className="mono text-[12px]">cargo run</code> in <code className="mono text-[12px]">backend/</code> and reload.
        </p>
      </div>
    );
  }

  const narrative: Narrative | null = brief?.narrative ?? null;
  const scale = Math.max(...facts.orders.map((o) => Math.max(o.p90, o.onhand, o.suggest_qty)), 10);

  async function printBrief() {
    setPrinting(true);
    setError(null);
    try {
      const resp = await postJSON<BriefResponse>(`/api/brief/${store}/generate?date=${date}`, {});
      setBrief(resp);
    } catch (e) {
      setError(String(e));
    } finally {
      setPrinting(false);
    }
  }

  async function decide(item: number, action: string, qty?: number) {
    try {
      const resp = await postJSON<{ entry: { hash: string } }>(`/api/decisions`, {
        store,
        brief_date: date,
        item,
        action,
        qty: qty ?? null,
        reason: { source: "brief", action },
      });
      setDecisions((d) => ({ ...d, [item]: { action, hash: resp.entry.hash } }));
    } catch (e) {
      setError(String(e));
    }
  }

  const t = facts.totals;

  return (
    <div>
      {/* ---------- edition line ---------- */}
      <div className="mt-5 flex flex-wrap items-baseline justify-between gap-2 border-b border-[var(--color-ink)] pb-2">
        <div className="text-[11px] tracking-[0.18em] uppercase">
          {fmt.day(facts.date)} · {facts.store_meta.city} · fresh section
        </div>
        <div className="mono text-[11px] text-[var(--color-muted)]">
          Store {facts.store} · brief for {facts.generated_for} · printed by {model || "—"}
        </div>
      </div>

      {error ? (
        <div className="rule-top mt-4 border border-[var(--color-red)] bg-[var(--color-red-soft)] p-3 text-[13px] text-[var(--color-red)]">
          {error}
        </div>
      ) : null}

      {/* ---------- lead ---------- */}
      <div className="mt-6 grid gap-8 lg:grid-cols-[1.6fr_1fr]">
        <div className="print-in">
          {narrative ? (
            <>
              <h1 className="display text-3xl leading-[1.08] font-semibold sm:text-[2.6rem]">{narrative.headline}</h1>
              <p className="mt-3 max-w-[62ch] font-[var(--font-display)] text-[15px] text-[var(--color-ink-2)] italic">
                {narrative.dek}
              </p>
            </>
          ) : (
            <>
              <h1 className="display text-3xl leading-[1.08] font-semibold sm:text-[2.6rem]">
                {t.order_lines} order lines · {t.markdown_lines} markdowns · {fmt.n0(t.at_risk_units)} units at risk
              </h1>
              <p className="mt-3 max-w-[62ch] text-[14px] text-[var(--color-muted)]">
                Numbers are ready. Words are one keystroke away.
              </p>
              <button
                type="button"
                onClick={printBrief}
                disabled={printing}
                className="mt-5 border border-[var(--color-ink)] bg-[var(--color-ink)] px-4 py-2 text-[11px] tracking-[0.14em] text-[var(--color-paper)] uppercase transition-opacity hover:opacity-85 disabled:opacity-50"
              >
                {printing ? "Setting type…" : "Print today's brief"}
              </button>
              {printing ? (
                <span className="ml-3 inline-flex gap-1 align-middle" aria-hidden>
                  <span className="press-dot inline-block h-1.5 w-1.5 rounded-full bg-[var(--color-ink)]" />
                  <span className="press-dot inline-block h-1.5 w-1.5 rounded-full bg-[var(--color-ink)]" />
                  <span className="press-dot inline-block h-1.5 w-1.5 rounded-full bg-[var(--color-ink)]" />
                </span>
              ) : (
                <span className="ml-3 text-[12px] text-[var(--color-muted)]">
                  language runs on the AMD-hosted model · every figure is precomputed
                </span>
              )}
            </>
          )}

          {brief?.grounding ? (
            <div className="mt-4 flex flex-wrap items-center gap-2">
              {brief.grounding.ok ? (
                <Chip tone={brief.grounding.source === "template_fallback" ? "ochre" : "green"}>
                  {brief.grounding.source === "template_fallback"
                    ? "template copy · checks passed"
                    : "figures verified"}
                </Chip>
              ) : (
                <Chip tone="red">grounding violations: {brief.grounding.violations.join(", ")}</Chip>
              )}
              <span className="mono text-[11px] text-[var(--color-muted)]">
                {brief.grounding.checked} figures checked against the ledger ·{" "}
                {brief.usage ? `${brief.usage.total_tokens} tokens` : ""} ·{" "}
                {brief.cost_usd != null ? `$${brief.cost_usd.toFixed(4)} this brief` : ""}
                {brief.cached ? " · from cache" : ""}
              </span>
            </div>
          ) : null}
        </div>

        {/* ---------- figure strip ---------- */}
        <div className="grid grid-cols-2 content-start gap-x-6 gap-y-5 self-start border-l border-[var(--color-rule)] pl-6">
          <Figure label="On hand" value={fmt.n0(t.onhand_units)} unit="units" />
          <Figure label="At risk ≤2 days" value={fmt.n0(t.at_risk_units)} unit="units" tone="ochre" />
          <Figure label="Orders due" value={fmt.n0(t.order_units)} unit={`units · ${t.order_lines} lines`} />
          <Figure label="Expected out today" value={fmt.n0(t.expected_sales_today)} unit="units" />
        </div>
      </div>

      {/* ---------- recap ---------- */}
      <div className="rule-top mt-8 grid gap-4 pt-4 sm:grid-cols-3">
        <div>
          <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">Yesterday</div>
          <div className="mono mt-1 text-[13px]">
            {fmt.n0(facts.recap.sales_units)} units sold · model said {fmt.n0(facts.recap.p50_units)}{" "}
            <span className={facts.recap.delta_pct >= 0 ? "text-[var(--color-green)]" : "text-[var(--color-red)]"}>
              {fmt.pct(facts.recap.delta_pct)}
            </span>
          </div>
        </div>
        <div>
          <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">Footfall</div>
          <div className="mono mt-1 text-[13px]">
            {fmt.n0(facts.recap.traffic)} transactions ·{" "}
            <span className={facts.recap.traffic_vs_7d_pct >= 0 ? "text-[var(--color-green)]" : "text-[var(--color-red)]"}>
              {fmt.pct(facts.recap.traffic_vs_7d_pct)}
            </span>{" "}
            vs 7-day
          </div>
        </div>
        <div>
          <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">Biggest miss</div>
          <div className="mono mt-1 text-[13px]">
            {facts.recap.top_miss_label} — {fmt.n0(facts.recap.top_miss_actual)} vs {fmt.n0(facts.recap.top_miss_p50)} forecast
          </div>
        </div>
      </div>

      {facts.holiday_tomorrow ? (
        <div className="rule-top mt-4 pt-3 text-[12.5px] text-[var(--color-ochre)]">
          Tomorrow is {facts.holiday_tomorrow} — the model already carries the holiday.
        </div>
      ) : null}

      <div className="mt-10 grid gap-10 lg:grid-cols-[1.65fr_1fr]">
        {/* ================= ORDERS ================= */}
        <section>
          <SectionHeading
            kicker="Section I"
            title="Orders for tomorrow"
            note={`Newsvendor target at ${fmt.n1(facts.service_level * 100)}% service level — arrivals before opening`}
          />
          {narrative ? <p className="mb-4 max-w-[70ch] text-[13.5px] text-[var(--color-ink-2)]">{narrative.orders_summary}</p> : null}
          {facts.orders.length === 0 ? (
            <p className="text-[13px] text-[var(--color-muted)]">
              No orders due for {facts.generated_for} — outside the forecast horizon.
            </p>
          ) : (
          <div className="border-t border-[var(--color-rule)]">
            {(showAllOrders ? facts.orders : facts.orders.slice(0, 12)).map((o: OrderLine) => {
              const d = decisions[o.item];
              return (
                <div key={o.item} className="grid grid-cols-[1fr_auto] gap-x-4 border-b border-[var(--color-rule-2)] py-2.5">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-baseline gap-x-2">
                      <span className="display text-[15px] font-medium">{o.label}</span>
                      {o.promo_today ? <Chip tone="green">on promo</Chip> : null}
                      {d ? <Chip tone="ink">{d.action} · {d.hash.slice(0, 8)}</Chip> : null}
                    </div>
                    <div className="mono mt-1 flex flex-wrap items-center gap-x-4 gap-y-1 text-[12px] text-[var(--color-ink-2)]">
                      <span>on hand {fmt.n0(o.onhand)}</span>
                      <Interval p10={o.p10} p50={o.p50} p90={o.p90} scale={scale} />
                      <span className="text-[var(--color-muted)]">p50 {fmt.n1(o.p50)}</span>
                      <span className="text-[var(--color-muted)]">
                        {o.cover_days > 900 ? "—" : `${fmt.n1(o.cover_days)}d cover`}
                      </span>
                    </div>
                    <Why chips={o.why} />
                  </div>
                  <div className="flex flex-col items-end gap-1">
                    <div className="mono text-[17px] font-semibold">{o.suggest_qty}</div>
                    <div className="mono text-[10.5px] text-[var(--color-muted)]">units</div>
                    {!d ? (
                      <div className="mt-1 flex gap-1">
                        <ActionButton label="Approve" tone="green" onClick={() => decide(o.item, "approve", o.suggest_qty)} />
                        <ActionButton
                          label="Edit"
                          tone="muted"
                          onClick={() => {
                            setEditItem(o.item);
                            setEditQty(String(o.suggest_qty));
                          }}
                        />
                        <ActionButton label="Flag" tone="red" onClick={() => decide(o.item, "flag", o.suggest_qty)} />
                      </div>
                    ) : null}
                    {editItem === o.item ? (
                      <div className="mt-1 flex items-center gap-1">
                        <input
                          value={editQty}
                          onChange={(e) => setEditQty(e.target.value)}
                          inputMode="numeric"
                          className="mono w-16 border border-[var(--color-ink)] bg-transparent px-1.5 py-0.5 text-[12px]"
                        />
                        <ActionButton
                          label="Save"
                          onClick={() => {
                            decide(o.item, "edit", Number(editQty) || 0);
                            setEditItem(null);
                          }}
                        />
                      </div>
                    ) : null}
                  </div>
                </div>
              );
            })}
            {facts.orders.length > 12 ? (
              <button
                type="button"
                onClick={() => setShowAllOrders((v) => !v)}
                className="mt-2 border border-[var(--color-rule)] px-3 py-1 text-[10.5px] tracking-[0.08em] text-[var(--color-muted)] uppercase hover:border-[var(--color-ink)] hover:text-[var(--color-ink)]"
              >
                {showAllOrders ? "Show the twelve largest" : `Show all ${facts.orders.length} lines`}
              </button>
            ) : null}
          </div>
          )}
        </section>

        {/* ================= SIDEBAR ================= */}
        <div className="space-y-10">
          <section>
            <SectionHeading
              kicker="Section II"
              title="Markdowns before expiry"
              note={`${facts.totals.markdown_lines} lines inside their last two days`}
            />
            {narrative ? <p className="mb-3 text-[13.5px] text-[var(--color-ink-2)]">{narrative.markdowns_summary}</p> : null}
            {facts.markdowns.length === 0 ? (
              <p className="text-[13px] text-[var(--color-muted)]">Nothing expires in the next two days.</p>
            ) : (
              <div className="border-t border-[var(--color-rule)]">
                {(showAllMarkdowns ? facts.markdowns : facts.markdowns.slice(0, 8)).map((m: MarkdownLine) => {
                  const d = decisions[m.item];
                  return (
                    <div key={m.item} className="border-b border-[var(--color-rule-2)] py-2.5">
                      <div className="flex items-baseline justify-between gap-3">
                        <span className="display text-[14.5px] font-medium">{m.label}</span>
                        <span className="mono text-[15px] font-semibold text-[var(--color-ochre)]">
                          −{Math.round(m.cut_pct * 100)}%
                        </span>
                      </div>
                      <div className="mono mt-1 text-[11.5px] text-[var(--color-ink-2)]">
                        {fmt.n0(m.onhand)} units · {m.expires_in_days <= 1 ? "expires today" : `expires in ${m.expires_in_days} days`} ·
                        clears ~{fmt.n0(m.expected_clear_units)} · recovers {fmt.usd(m.recovered_usd)}
                      </div>
                      <div className="mt-1.5 flex items-center gap-1">
                        {!d ? (
                          <>
                            <ActionButton label="Mark down" tone="green" onClick={() => decide(m.item, "approve", m.onhand)} />
                            <ActionButton label="Leave" tone="muted" onClick={() => decide(m.item, "reject", m.onhand)} />
                          </>
                        ) : (
                          <Chip tone="ink">{d.action} · {d.hash.slice(0, 8)}</Chip>
                        )}
                      </div>
                    </div>
                  );
                })}
                {facts.markdowns.length > 8 ? (
                  <button
                    type="button"
                    onClick={() => setShowAllMarkdowns((v) => !v)}
                    className="mt-2 border border-[var(--color-rule)] px-3 py-1 text-[10.5px] tracking-[0.08em] text-[var(--color-muted)] uppercase hover:border-[var(--color-ink)] hover:text-[var(--color-ink)]"
                  >
                    {showAllMarkdowns ? "Show the eight most urgent" : `Show all ${facts.markdowns.length} lines`}
                  </button>
                ) : null}
              </div>
            )}
          </section>

          <section>
            <SectionHeading kicker="Section III" title="Promotions" note="measured lift from the store's own promo history" />
            {narrative ? <p className="mb-3 text-[13.5px] text-[var(--color-ink-2)]">{narrative.promos_summary}</p> : null}
            {facts.promos.length === 0 ? (
              <p className="text-[13px] text-[var(--color-muted)]">No promo candidates today.</p>
            ) : (
              <div className="border-t border-[var(--color-rule)]">
                {facts.promos.map((p: PromoLine) => {
                  const creative = narrative?.promo_creative?.find((c) => c.item === p.item);
                  return (
                    <div key={p.item} className="border-b border-[var(--color-rule-2)] py-2.5">
                      <div className="flex items-baseline justify-between gap-2">
                        <span className="display text-[14.5px] font-medium">{p.label}</span>
                        <span className="mono text-[12px] text-[var(--color-green)]">lift {fmt.n0(p.lift_pct)}%</span>
                      </div>
                      <div className="mono mt-1 text-[11.5px] text-[var(--color-ink-2)]">
                        {fmt.n0(p.onhand)} units · {p.cover_days > 900 ? "—" : `${fmt.n1(p.cover_days)}d cover`} · ~
                        {fmt.n1(p.expected_incremental_units)} extra units
                      </div>
                      {creative ? (
                        <div className="mt-2 border-l-2 border-[var(--color-green)] pl-2.5">
                          <div className="display text-[13.5px] font-medium">{creative.headline}</div>
                          <p className="text-[12.5px] text-[var(--color-ink-2)]">{creative.body}</p>
                        </div>
                      ) : null}
                      {!decisions[p.item] ? (
                        <div className="mt-1.5">
                          <ActionButton label="Run promo" tone="green" onClick={() => decide(p.item, "approve", p.expected_incremental_units)} />
                        </div>
                      ) : (
                        <div className="mt-1.5">
                          <Chip tone="ink">{decisions[p.item].action} · {decisions[p.item].hash.slice(0, 8)}</Chip>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </section>

          <section>
            <SectionHeading kicker="Section IV" title="Watchlist" note="where the model is least certain today" />
            {narrative ? <p className="mb-3 text-[13.5px] text-[var(--color-ink-2)]">{narrative.watchlist_summary}</p> : null}
            <ul className="space-y-2">
              {facts.watchlist.map((w) => (
                <li key={w.item} className="flex items-start gap-2 text-[13px]">
                  <Chip tone={w.risk === "stockout" ? "red" : "ochre"}>{w.risk}</Chip>
                  <span>
                    <span className="display font-medium">{w.label}</span>{" "}
                    <span className="mono text-[12px] text-[var(--color-muted)]">{w.detail}</span>
                  </span>
                </li>
              ))}
            </ul>
          </section>

          <section className="rule-top pt-4">
            <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">Method</div>
            <p className="mt-1 text-[12px] text-[var(--color-muted)]">
              Orders come from quantile demand forecasts (LightGBM p10/p50/p90) validated out-of-sample on
              held-out weeks. Inventory, expiry and the replay run in a deterministic simulator —{" "}
              <Link href="/trust" className="underline decoration-[var(--color-rule)] underline-offset-2 hover:decoration-[var(--color-ink)]">
                see the numbers
              </Link>
              {" · "}
              <Link href="/replay" className="underline decoration-[var(--color-rule)] underline-offset-2 hover:decoration-[var(--color-ink)]">
                run the replay
              </Link>
              .
            </p>
          </section>
        </div>
      </div>
    </div>
  );
}

function Figure({
  label,
  value,
  unit,
  tone = "ink",
}: {
  label: string;
  value: string;
  unit: string;
  tone?: "ink" | "ochre";
}) {
  return (
    <div>
      <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">{label}</div>
      <div className={`mono mt-1 text-[1.7rem] leading-none font-semibold ${tone === "ochre" ? "text-[var(--color-ochre)]" : ""}`}>
        {value}
      </div>
      <div className="mt-0.5 text-[11px] text-[var(--color-muted)]">{unit}</div>
    </div>
  );
}
