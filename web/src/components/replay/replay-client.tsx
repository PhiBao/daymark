"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { ReplayResponse, SimOutcome, fmt, getJSON } from "@/lib/api";

type Preset = "4w" | "full";

const POLICY_COLORS: Record<string, string> = {
  spreadsheet: "var(--color-muted)",
  forecast_only: "var(--color-ochre)",
  daymark: "var(--color-green)",
};

const POLICY_DASH: Record<string, string> = {
  spreadsheet: "4 3",
  forecast_only: "1 3",
  daymark: "none",
};

function TapeChart({
  title,
  unit,
  outcomes,
  pick,
  format,
  highlight,
}: {
  title: string;
  unit: string;
  outcomes: SimOutcome[];
  pick: (o: SimOutcome) => number[];
  format: (v: number) => string;
  highlight?: boolean;
}) {
  const series = outcomes.map((o) => ({ id: o.policy, values: pick(o) }));
  const width = 640;
  const height = 120;
  const pad = 6;
  const max = Math.max(1, ...series.flatMap((s) => s.values));
  const n = Math.max(...series.map((s) => s.values.length), 2);
  const path = (vals: number[]) =>
    vals
      .map((v, i) => {
        const x = pad + (i / (n - 1)) * (width - pad * 2);
        const y = height - pad - (v / max) * (height - pad * 2);
        return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");
  return (
    <div className={highlight ? "border border-[var(--color-green)] p-3" : "border border-[var(--color-rule-2)] p-3"}>
      <div className="flex items-baseline justify-between">
        <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">{title}</div>
        <div className="mono text-[10px] text-[var(--color-muted)]">{unit} · peak {format(max)}</div>
      </div>
      <svg viewBox={`0 0 ${width} ${height}`} className="mt-1 w-full" role="img" aria-label={title}>
        {[0.25, 0.5, 0.75].map((f) => (
          <line
            key={f}
            x1={0}
            x2={width}
            y1={pad + f * (height - pad * 2)}
            y2={pad + f * (height - pad * 2)}
            stroke="var(--color-rule-2)"
            strokeWidth={1}
          />
        ))}
        {series.map((s) => (
          <path
            key={s.id}
            d={path(s.values)}
            fill="none"
            stroke={POLICY_COLORS[s.id]}
            strokeWidth={s.id === "daymark" ? 2.4 : 1.6}
            strokeDasharray={POLICY_DASH[s.id]}
            className={s.id === "daymark" ? "tape-run" : undefined}
            style={{ ["--tape-len" as string]: "3000" }}
          />
        ))}
      </svg>
      <div className="mt-1 flex flex-wrap gap-x-4 gap-y-1">
        {outcomes.map((o) => (
          <span key={o.policy} className="inline-flex items-center gap-1.5 text-[11px] text-[var(--color-muted)]">
            <span
              aria-hidden
              className="inline-block h-[3px] w-4"
              style={{ background: POLICY_COLORS[o.policy] }}
            />
            {o.policy_label}
          </span>
        ))}
      </div>
    </div>
  );
}

function BigDelta({
  label,
  value,
  good,
  sub,
}: {
  label: string;
  value: string;
  good: boolean;
  sub: string;
}) {
  return (
    <div className="border-l border-[var(--color-rule)] pl-4">
      <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">{label}</div>
      <div className={`mono mt-1 text-[1.5rem] leading-none font-semibold ${good ? "text-[var(--color-green)]" : "text-[var(--color-red)]"}`}>
        {value}
      </div>
      <div className="mt-0.5 text-[11px] text-[var(--color-muted)]">{sub}</div>
    </div>
  );
}

export default function ReplayClient({ store }: { store: number }) {
  const [preset, setPreset] = useState<Preset>("full");
  const [serviceLevel, setServiceLevel] = useState(0.92);
  const [liftScale, setLiftScale] = useState(1.0);
  const [shelfScale, setShelfScale] = useState(1.0);
  const [data, setData] = useState<ReplayResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const q = new URLSearchParams({
        store: String(store),
        service_level: serviceLevel.toFixed(3),
        lift_scale: liftScale.toFixed(3),
        shelf_life_scale: shelfScale.toFixed(3),
      });
      if (preset === "4w" && data) {
        // last 28 days of the default window
        const end = data.window.end;
        const start = new Date(new Date(`${end}T12:00:00Z`).getTime() - 27 * 864e5)
          .toISOString()
          .slice(0, 10);
        q.set("start", start);
        q.set("end", end);
      }
      const resp = await getJSON<ReplayResponse>(`/api/replay?${q.toString()}`);
      setData(resp);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [store, preset, serviceLevel, liftScale, shelfScale, data]);

  useEffect(() => {
    void (async () => {
      setLoading(true);
      try {
        const q = new URLSearchParams({
          store: String(store),
          service_level: serviceLevel.toFixed(3),
          lift_scale: liftScale.toFixed(3),
          shelf_life_scale: shelfScale.toFixed(3),
        });
        const resp = await getJSON<ReplayResponse>(`/api/replay?${q.toString()}`);
        setData(resp);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [store]);

  const outcomeById = useMemo(() => {
    const map: Record<string, SimOutcome> = {};
    data?.policies.forEach((p) => (map[p.policy] = p));
    return map;
  }, [data]);

  const d = data?.deltas.daymark_vs_spreadsheet;
  const dForecast = data?.deltas.daymark_vs_forecast_only;

  return (
    <div>
      <div className="mt-6 border-b border-[var(--color-ink)] pb-2 text-[11px] tracking-[0.18em] uppercase">
        Replay · exactly what would have happened · store {store}
      </div>

      <div className="mt-6 grid gap-8 lg:grid-cols-[1.7fr_1fr]">
        <div>
          <h1 className="display text-3xl leading-[1.08] font-semibold sm:text-[2.4rem]">
            The last ten weeks, run three ways.
          </h1>
          <p className="mt-3 max-w-[68ch] text-[14px] text-[var(--color-ink-2)]">
            The same recorded demand, the same opening shelves, the same shelf life — one spreadsheet, one
            forecast without markdowns, one full Daymark policy. Every unit below is counted by the simulator,
            arithmetic you can re-run with a single request.
          </p>
        </div>
        <div className="content-start self-start border-l border-[var(--color-rule)] pl-5 text-[12px] text-[var(--color-muted)]">
          <div className="text-[10px] tracking-[0.16em] uppercase">Method, honestly</div>
          <p className="mt-1">
            Demand = recorded sales from the public Favorita benchmark. Stock-out censoring is a known
            limitation; shelf life and unit prices are declared assumptions. Sensitivity is in your hands —
            move the dials.
          </p>
        </div>
      </div>

      {/* controls */}
      <div className="rule-top mt-8 grid gap-6 pt-4 lg:grid-cols-[auto_1fr_auto] lg:items-end">
        <div className="flex flex-wrap items-center gap-2">
          <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">Window</div>
          <div className="flex border border-[var(--color-rule)]">
            {(["full", "4w"] as Preset[]).map((p) => (
              <button
                key={p}
                type="button"
                onClick={() => setPreset(p)}
                className={`px-3 py-1 text-[11px] tracking-[0.08em] uppercase ${
                  preset === p ? "bg-[var(--color-ink)] text-[var(--color-paper)]" : "text-[var(--color-ink-2)]"
                }`}
              >
                {p === "full" ? "Full backtest" : "Last 4 weeks"}
              </button>
            ))}
          </div>
        </div>
        <div className="grid gap-4 sm:grid-cols-3">
          <label className="block">
            <div className="flex justify-between text-[10px] tracking-[0.14em] text-[var(--color-muted)] uppercase">
              <span>Service level</span>
              <span className="mono">{serviceLevel.toFixed(2)}</span>
            </div>
            <input
              type="range"
              min={0.8}
              max={0.98}
              step={0.01}
              value={serviceLevel}
              onChange={(e) => setServiceLevel(Number(e.target.value))}
              className="w-full accent-[var(--color-ink)]"
            />
          </label>
          <label className="block">
            <div className="flex justify-between text-[10px] tracking-[0.14em] text-[var(--color-muted)] uppercase">
              <span>Markdown lift ×</span>
              <span className="mono">{liftScale.toFixed(2)}</span>
            </div>
            <input
              type="range"
              min={0}
              max={2}
              step={0.05}
              value={liftScale}
              onChange={(e) => setLiftScale(Number(e.target.value))}
              className="w-full accent-[var(--color-ink)]"
            />
          </label>
          <label className="block">
            <div className="flex justify-between text-[10px] tracking-[0.14em] text-[var(--color-muted)] uppercase">
              <span>Shelf life ×</span>
              <span className="mono">{shelfScale.toFixed(2)}</span>
            </div>
            <input
              type="range"
              min={0.5}
              max={1.5}
              step={0.05}
              value={shelfScale}
              onChange={(e) => setShelfScale(Number(e.target.value))}
              className="w-full accent-[var(--color-ink)]"
            />
          </label>
        </div>
        <button
          type="button"
          onClick={run}
          disabled={loading}
          className="border border-[var(--color-ink)] bg-[var(--color-ink)] px-4 py-2 text-[11px] tracking-[0.14em] text-[var(--color-paper)] uppercase transition-opacity hover:opacity-85 disabled:opacity-50"
        >
          {loading ? "Running…" : "Run replay"}
        </button>
      </div>

      {error ? (
        <div className="rule-top mt-6 border border-[var(--color-red)] bg-[var(--color-red-soft)] p-3 text-[13px] text-[var(--color-red)]">
          {error}
        </div>
      ) : null}

      {data ? (
        <>
          {/* headline deltas */}
          <div className="rule-top mt-8 grid grid-cols-2 gap-6 pt-5 sm:grid-cols-4">
            <BigDelta
              label="Waste avoided"
              value={d ? `${d.waste_units_delta_pct <= 0 ? "−" : "+"}${Math.abs(d.waste_units_delta_pct).toFixed(0)}%` : "—"}
              good={(d?.waste_units_delta_pct ?? 0) <= 0}
              sub={d ? `${fmt.n0(Math.abs(d.waste_units_delta))} units vs spreadsheet` : ""}
            />
            <BigDelta
              label="Lost sales"
              value={d ? `${d.lost_units_delta_pct <= 0 ? "−" : "+"}${Math.abs(d.lost_units_delta_pct).toFixed(0)}%` : "—"}
              good={(d?.lost_units_delta_pct ?? 0) <= 0}
              sub={d ? `${fmt.n0(Math.abs(d.lost_units_delta))} units vs spreadsheet` : ""}
            />
            <BigDelta
              label="Service level"
              value={d ? `${d.service_level_delta_pts >= 0 ? "+" : ""}${d.service_level_delta_pts.toFixed(1)} pts` : "—"}
              good={(d?.service_level_delta_pts ?? 0) >= 0}
              sub="share of demand served"
            />
            <BigDelta
              label="vs forecast-only"
              value={dForecast ? `${dForecast.waste_units_delta_pct <= 0 ? "−" : "+"}${Math.abs(dForecast.waste_units_delta_pct).toFixed(0)}%` : "—"}
              good={(dForecast?.waste_units_delta_pct ?? 0) <= 0}
              sub="isolates the markdown engine"
            />
          </div>

          {/* policy table */}
          <div className="rule-top mt-8 overflow-x-auto pt-4">
            <table className="w-full min-w-[640px] border-collapse text-[13px]">
              <thead>
                <tr className="text-left text-[10px] tracking-[0.14em] text-[var(--color-muted)] uppercase">
                  <th className="py-2 pr-4 font-normal">Policy</th>
                  <th className="py-2 pr-4 text-right font-normal">Waste units</th>
                  <th className="py-2 pr-4 text-right font-normal">Lost units</th>
                  <th className="py-2 pr-4 text-right font-normal">Service</th>
                  <th className="py-2 pr-4 text-right font-normal">Revenue</th>
                  <th className="py-2 text-right font-normal">Waste value</th>
                </tr>
              </thead>
              <tbody className="mono">
                {data.policies.map((p) => (
                  <tr
                    key={p.policy}
                    className={p.policy === "daymark" ? "border-y-2 border-[var(--color-green)]" : "border-b border-[var(--color-rule-2)]"}
                  >
                    <td className="py-2.5 pr-4 font-[var(--font-sans)]">
                      <span className="display text-[14.5px] font-medium">{p.policy_label}</span>
                      {p.policy === "daymark" ? (
                        <span className="ml-2 border border-[var(--color-green)] px-1.5 py-0.5 text-[10px] tracking-[0.08em] text-[var(--color-green)] uppercase">
                          this brief
                        </span>
                      ) : null}
                    </td>
                    <td className="py-2.5 pr-4 text-right">{fmt.n0(p.totals.waste_units)}</td>
                    <td className="py-2.5 pr-4 text-right">{fmt.n0(p.totals.lost_units)}</td>
                    <td className="py-2.5 pr-4 text-right">{p.totals.service_level_pct.toFixed(1)}%</td>
                    <td className="py-2.5 pr-4 text-right">{fmt.usd(p.totals.revenue_usd)}</td>
                    <td className="py-2.5 text-right">{fmt.usd(p.totals.waste_value_usd)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* tapes */}
          <div className="mt-8 grid gap-4 md:grid-cols-3">
            <TapeChart
              title="Waste per day"
              unit="units"
              outcomes={data.policies}
              pick={(o) => o.daily.map((d) => d.waste_units)}
              format={(v) => fmt.n1(v)}
              highlight
            />
            <TapeChart
              title="Lost sales per day"
              unit="units"
              outcomes={data.policies}
              pick={(o) => o.daily.map((d) => d.lost_units)}
              format={(v) => fmt.n1(v)}
            />
            <TapeChart
              title="Shelf stock at close"
              unit="units"
              outcomes={data.policies}
              pick={(o) => o.daily.map((d) => d.onhand_end)}
              format={(v) => fmt.n0(v)}
            />
          </div>

          {/* by family */}
          <div className="mt-10 grid gap-8 lg:grid-cols-2">
            <div>
              <div className="rule-top mb-3 pt-3">
                <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">Breakdown</div>
                <h2 className="display text-[1.3rem] font-semibold">Waste by category</h2>
              </div>
              <table className="w-full border-collapse text-[13px]">
                <thead>
                  <tr className="text-left text-[10px] tracking-[0.14em] text-[var(--color-muted)] uppercase">
                    <th className="py-2 font-normal">Category</th>
                    <th className="py-2 text-right font-normal">Spreadsheet</th>
                    <th className="py-2 text-right font-normal">Daymark</th>
                    <th className="py-2 text-right font-normal">Δ</th>
                  </tr>
                </thead>
                <tbody className="mono">
                  {(outcomeById["daymark"]?.by_family ?? []).slice(0, 7).map((f) => {
                    const base = outcomeById["spreadsheet"]?.by_family.find((x) => x.family === f.family);
                    const delta = (base?.waste_units ?? 0) - f.waste_units;
                    return (
                      <tr key={f.family} className="border-b border-[var(--color-rule-2)]">
                        <td className="py-2 font-[var(--font-sans)]">{f.family}</td>
                        <td className="py-2 text-right">{fmt.n0(base?.waste_units ?? 0)}</td>
                        <td className="py-2 text-right">{fmt.n0(f.waste_units)}</td>
                        <td className={`py-2 text-right ${delta >= 0 ? "text-[var(--color-green)]" : "text-[var(--color-red)]"}`}>
                          {delta >= 0 ? "−" : "+"}
                          {fmt.n0(Math.abs(delta))}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            <details className="rule-top pt-3 text-[12.5px] text-[var(--color-muted)]">
              <summary className="cursor-pointer text-[10px] tracking-[0.16em] uppercase">Reproduce this exact replay</summary>
              <pre className="mono mt-2 overflow-x-auto border border-[var(--color-rule)] bg-[var(--color-paper-2)] p-3 text-[11px] text-[var(--color-ink-2)]">
{`curl "http://localhost:8787/api/replay?store=${store}\\
  &start=${data.window.start}&end=${data.window.end}\\
  &service_level=${data.params.service_level}\\
  &lift_scale=${data.params.lift_scale}\\
  &shelf_life_scale=${data.params.shelf_life_scale}"`}
              </pre>
              <p className="mt-2">{data.note}</p>
              <p className="mt-2">
                Spreadsheet policy: order = last same-weekday sales × 1.05, no markdowns. Forecast-only: newsvendor
                target, no markdowns. Daymark: newsvendor target + markdowns inside the last two days of shelf
                life at {fmt.n0(30)}% / {fmt.n0(50)}% cuts.
              </p>
            </details>
          </div>
        </>
      ) : loading ? (
        <div className="mono mt-10 text-[13px] text-[var(--color-muted)]">Running three policies across the window…</div>
      ) : null}
    </div>
  );
}
