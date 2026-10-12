import { TrustResponse, fmt, getJSON } from "@/lib/api";

export const instant = false;


function Section({ kicker, title, children }: { kicker: string; title: string; children: React.ReactNode }) {
  return (
    <section className="rule-top mt-8 pt-4">
      <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">{kicker}</div>
      <h2 className="display mb-3 text-[1.35rem] font-semibold">{title}</h2>
      {children}
    </section>
  );
}

export default async function Page() {
  let trust: TrustResponse | null = null;
  let error: string | null = null;
  try {
    trust = await getJSON<TrustResponse>(`/api/trust`);
  } catch (e) {
    error = String(e);
  }

  if (!trust) {
    return (
      <div className="rule-top mt-8 border border-[var(--color-rule)] bg-[var(--color-paper-2)] p-8 text-center">
        <div className="display text-2xl">No numbers to show yet.</div>
        <p className="mt-2 text-[13.5px] text-[var(--color-muted)]">{error}</p>
      </div>
    );
  }

  const o = trust.eval_metrics.overall as Record<string, number>;
  const families = Object.entries(trust.eval_metrics.by_family ?? {}).sort(
    (a, b) => (b[1].n ?? 0) - (a[1].n ?? 0),
  );
  const lift = trust.promo_lift;
  const bench = trust.benchmark as Record<string, unknown> | null;
  const assumptions = (trust.assumptions ?? {}) as {
    shelf_life_days_by_family?: Record<string, number>;
    unit_price_usd_by_family?: Record<string, number>;
    note?: string;
  };

  return (
    <div>
      <div className="mt-6 border-b border-[var(--color-ink)] pb-2 text-[11px] tracking-[0.18em] uppercase">
        Trust · the receipts behind every figure
      </div>

      <div className="mt-6 grid gap-8 lg:grid-cols-[1.7fr_1fr]">
        <div>
          <h1 className="display text-3xl leading-[1.08] font-semibold sm:text-[2.4rem]">
            If it can&apos;t be checked, it isn&apos;t written.
          </h1>
          <p className="mt-3 max-w-[68ch] text-[14px] text-[var(--color-ink-2)]">
            Forecast skill measured out-of-sample on held-out weeks, a serving benchmark from the AMD GPU, the
            measured promotion lift the markdown engine borrows from, and the declared assumptions — all in one
            place.
          </p>
        </div>
        <div className="content-start self-start border-l border-[var(--color-rule)] pl-5 text-[12.5px] text-[var(--color-muted)]">
          <div className="text-[10px] tracking-[0.16em] uppercase">Grounding</div>
          <p className="mt-1">{trust.grounding}</p>
        </div>
      </div>

      <Section kicker="I" title={`Demand forecast — out-of-sample ${trust.eval_metrics.window?.[0]} to ${trust.eval_metrics.window?.[1] ?? ""}`}>
        <div className="grid grid-cols-2 gap-6 sm:grid-cols-4">
          <Stat label="WAPE (p50)" value={`${o.wape_p50_pct}%`} sub={`${fmt.n0(o.n)} item-days`} />
          <Stat label="Skill vs same-weekday" value={`${o.skill_vs_naive7_pct}%`} sub={`baseline ${o.baseline_naive7_wape_pct}%`} good />
          <Stat label="Band coverage p10–p90" value={`${o.coverage_p10_p90_pct}%`} sub="target 80%" good={Math.abs(o.coverage_p10_p90_pct - 80) < 8} />
          <Stat label="Bias" value={`${o.bias_pct}%`} sub="sum(forecast) vs sum(actual)" good={Math.abs(o.bias_pct) < 5} />
        </div>
        <div className="mt-5 overflow-x-auto">
          <table className="w-full min-w-[640px] border-collapse text-[13px]">
            <thead>
              <tr className="text-left text-[10px] tracking-[0.14em] text-[var(--color-muted)] uppercase">
                <th className="py-2 pr-4 font-normal">Category</th>
                <th className="py-2 pr-4 text-right font-normal">Item-days</th>
                <th className="py-2 pr-4 text-right font-normal">WAPE</th>
                <th className="py-2 pr-4 text-right font-normal">Baseline (same weekday)</th>
                <th className="py-2 pr-4 text-right font-normal">Skill</th>
                <th className="py-2 text-right font-normal">Coverage</th>
              </tr>
            </thead>
            <tbody className="mono">
              {families.map(([fam, m]) => (
                <tr key={fam} className="border-b border-[var(--color-rule-2)]">
                  <td className="py-2 pr-4 font-[var(--font-sans)]">{fam}</td>
                  <td className="py-2 pr-4 text-right">{fmt.n0(m.n ?? 0)}</td>
                  <td className="py-2 pr-4 text-right">{m.wape_p50_pct}%</td>
                  <td className="py-2 pr-4 text-right">{m.baseline_naive7_wape_pct}%</td>
                  <td className={`py-2 pr-4 text-right ${(m.skill_vs_naive7_pct ?? 0) >= 0 ? "text-[var(--color-green)]" : "text-[var(--color-red)]"}`}>
                    {m.skill_vs_naive7_pct}%
                  </td>
                  <td className="py-2 text-right">{m.coverage_p10_p90_pct}%</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Section>

      <div className="grid gap-10 lg:grid-cols-2">
        <Section kicker="II" title="Measured promotion lift (real history)">
          <p className="mb-3 text-[12.5px] text-[var(--color-muted)]">{lift.note}</p>
          <table className="w-full border-collapse text-[13px]">
            <thead>
              <tr className="text-left text-[10px] tracking-[0.14em] text-[var(--color-muted)] uppercase">
                <th className="py-2 font-normal">Category</th>
                <th className="py-2 text-right font-normal">Median lift</th>
                <th className="py-2 text-right font-normal">p25–p75</th>
                <th className="py-2 text-right font-normal">n</th>
              </tr>
            </thead>
            <tbody className="mono">
              <tr className="border-y-2 border-[var(--color-ink)]">
                <td className="py-2 font-[var(--font-sans)]">All categories</td>
                <td className="py-2 text-right">{lift.overall_pct}%</td>
                <td className="py-2 text-right">—</td>
                <td className="py-2 text-right">{fmt.n0(lift.n)}</td>
              </tr>
              {Object.entries(lift.by_family ?? {}).map(([fam, m]) => (
                <tr key={fam} className="border-b border-[var(--color-rule-2)]">
                  <td className="py-2 font-[var(--font-sans)]">{fam}</td>
                  <td className="py-2 text-right">{m.median_pct}%</td>
                  <td className="py-2 text-right">
                    {m.p25_pct}% to {m.p75_pct}%
                  </td>
                  <td className="py-2 text-right">{fmt.n0(m.n)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>

        <div>
          <Section kicker="III" title="Serving on AMD">
            {bench ? (
              <div className="grid grid-cols-2 gap-5">
                {Object.entries(bench)
                  .filter(([, v]) => typeof v === "number" || typeof v === "string")
                  .slice(0, 8)
                  .map(([k, v]) => (
                    <div key={k}>
                      <div className="text-[10px] tracking-[0.14em] text-[var(--color-muted)] uppercase">
                        {k.replaceAll("_", " ")}
                      </div>
                      <div className="mono mt-0.5 text-[15px] font-medium">
                        {typeof v === "number" ? fmt.n1(v) : String(v)}
                      </div>
                    </div>
                  ))}
              </div>
            ) : (
              <p className="text-[13px] text-[var(--color-muted)]">
                Benchmark not attached yet. Run <code className="mono text-[12px]">engine/daymark/bench.py</code>{" "}
                against the AMD endpoint to record tokens/s, TTFT and cost per brief.
              </p>
            )}
            <div className="mt-4 border-l border-[var(--color-rule)] pl-4 text-[12.5px] text-[var(--color-muted)]">
              <div className="text-[10px] tracking-[0.16em] uppercase">Cost model</div>
              <p className="mono mt-1 text-[12px] text-[var(--color-ink-2)]">
                {trust.cost_model.gpu_name} · ${trust.cost_model.gpu_hourly_usd}/gpu-hour ·{" "}
                {fmt.n1(trust.cost_model.tokens_per_sec)} tok/s ({trust.cost_model.source}) → $
                {trust.cost_model.usd_per_1m_tokens.toFixed(2)} per million tokens
              </p>
            </div>
          </Section>

          <Section kicker="IV" title="Declared assumptions">
            <p className="text-[12.5px] text-[var(--color-muted)]">{assumptions.note}</p>
            <div className="mt-3 grid grid-cols-2 gap-x-6 gap-y-2 text-[12.5px]">
              {Object.entries(assumptions.shelf_life_days_by_family ?? {}).map(([fam, d]) => (
                <div key={fam} className="flex justify-between border-b border-[var(--color-rule-2)] pb-1">
                  <span>{fam}</span>
                  <span className="mono">{d} days</span>
                </div>
              ))}
            </div>
          </Section>
        </div>
      </div>

      <Section kicker="V" title="Provenance">
        <p className="text-[12.5px] text-[var(--color-ink-2)]">{trust.source}</p>
      </Section>
    </div>
  );
}

function Stat({ label, value, sub, good }: { label: string; value: string; sub: string; good?: boolean }) {
  return (
    <div>
      <div className="text-[10px] tracking-[0.14em] text-[var(--color-muted)] uppercase">{label}</div>
      <div className={`mono mt-1 text-[1.5rem] leading-none font-semibold ${good ? "text-[var(--color-green)]" : ""}`}>
        {value}
      </div>
      <div className="mt-0.5 text-[11px] text-[var(--color-muted)]">{sub}</div>
    </div>
  );
}
