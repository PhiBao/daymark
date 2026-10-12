// Typed client for the Daymark API.

export const API_URL =
  process.env.NEXT_PUBLIC_API_URL ?? "http://127.0.0.1:8787";

export type WhyChip = { feature: string; label: string; contrib: number };

export type OrderLine = {
  item: number;
  label: string;
  family: string;
  onhand: number;
  cover_days: number;
  p10: number;
  p50: number;
  p90: number;
  suggest_qty: number;
  unit_price_usd: number;
  shelf_life_days: number;
  promo_today: boolean;
  why: WhyChip[];
};

export type MarkdownLine = {
  item: number;
  label: string;
  family: string;
  onhand: number;
  expires_in_days: number;
  cut_pct: number;
  expected_clear_units: number;
  recovered_usd: number;
  waste_avoided_usd: number;
  why: WhyChip[];
};

export type PromoLine = {
  item: number;
  label: string;
  family: string;
  lift_pct: number;
  cover_days: number;
  onhand: number;
  expected_incremental_units: number;
  why: WhyChip[];
};

export type WatchLine = { item: number; label: string; risk: string; detail: string };

export type Recap = {
  day: string;
  sales_units: number;
  p50_units: number;
  delta_pct: number;
  traffic: number;
  traffic_vs_7d_pct: number;
  top_miss_item: number;
  top_miss_label: string;
  top_miss_actual: number;
  top_miss_p50: number;
};

export type BriefTotals = {
  onhand_units: number;
  at_risk_units: number;
  expected_sales_today: number;
  order_lines: number;
  order_units: number;
  markdown_lines: number;
  recovered_usd: number;
  waste_avoided_usd: number;
  promo_lines: number;
  expected_incremental_units: number;
};

export type BriefFacts = {
  store: number;
  date: string;
  weekday: string;
  generated_for: string;
  store_meta: { store_nbr: number; city: string; state: string; store_type: string; cluster: number };
  holiday_today: string | null;
  holiday_tomorrow: string | null;
  service_level: number;
  recap: Recap;
  totals: BriefTotals;
  orders: OrderLine[];
  markdowns: MarkdownLine[];
  promos: PromoLine[];
  watchlist: WatchLine[];
  assumptions: Record<string, unknown>;
};

export type PromoCreative = { item: number; headline: string; body: string };

export type Narrative = {
  headline: string;
  dek: string;
  orders_summary: string;
  markdowns_summary: string;
  promos_summary: string;
  watchlist_summary: string;
  promo_creative: PromoCreative[];
};

export type GroundingReport = {
  ok: boolean;
  checked: number;
  violations: string[];
  source: string;
};

export type LlmUsage = { prompt_tokens: number; completion_tokens: number; total_tokens: number };

export type BriefResponse = {
  facts: BriefFacts;
  narrative: Narrative | null;
  grounding: GroundingReport | null;
  usage: LlmUsage | null;
  cost_usd: number | null;
  model: string;
  source: string;
  generated_at: string;
  cached: boolean;
};

export type Totals = {
  sales_units: number;
  lost_units: number;
  waste_units: number;
  waste_value_usd: number;
  revenue_usd: number;
  service_level_pct: number;
  demand_units: number;
  markdown_units: number;
  markdowns_triggered: number;
  orders_placed: number;
};

export type DayStat = {
  date: string;
  sales_units: number;
  lost_units: number;
  waste_units: number;
  waste_value_usd: number;
  revenue_usd: number;
  onhand_end: number;
};

export type FamilyStat = {
  family: string;
  demand_units: number;
  sales_units: number;
  lost_units: number;
  waste_units: number;
  waste_value_usd: number;
  service_level_pct: number;
};

export type SimOutcome = {
  policy: string;
  policy_label: string;
  daily: DayStat[];
  totals: Totals;
  by_family: FamilyStat[];
};

export type ReplayResponse = {
  store: number;
  window: { start: string; end: string; days: number };
  params: { service_level: number; lift_scale: number; shelf_life_scale: number };
  policies: SimOutcome[];
  deltas: {
    daymark_vs_spreadsheet: Record<string, number>;
    daymark_vs_forecast_only: Record<string, number>;
  };
  note: string;
};

export type LedgerEntry = {
  id: number;
  ts: string;
  store: number;
  brief_date: string;
  item: number | null;
  action: string;
  qty: number | null;
  reason: Record<string, unknown> | null;
  prev_hash: string;
  hash: string;
  chain_ok: boolean;
};

export type Health = {
  ok: boolean;
  artifacts: { forecast_window: [string, string]; stores: number[]; items: number };
  llm: { base_url: string; model: string; reachable: boolean; reported_model: string | null };
};

export type Meta = {
  meta: Record<string, unknown>;
  eval_metrics: { overall: Record<string, number>; by_family: Record<string, Record<string, number>>; window: [string, string] };
  promo_lift: { overall_pct: number; n: number; by_family: Record<string, { median_pct: number; p25_pct: number; p75_pct: number; n: number }>; note: string };
  benchmark: Record<string, unknown> | null;
  cost_model: { gpu_name: string; gpu_hourly_usd: number; tokens_per_sec: number; source: string; usd_per_1m_tokens: number };
  llm_model: string;
  stores: { store_nbr: number; city: string; state: string; store_type: string; cluster: number }[];
  families: string[];
};

export type TrustResponse = {
  eval_metrics: Meta["eval_metrics"];
  promo_lift: Meta["promo_lift"];
  benchmark: Record<string, unknown> | null;
  assumptions: Record<string, unknown> | null;
  source: string | null;
  cost_model: Meta["cost_model"];
  model: string;
  grounding: string;
};

async function read<T>(res: Response): Promise<T> {
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`${res.status} ${res.statusText}: ${body.slice(0, 200)}`);
  }
  return (await res.json()) as T;
}

export async function getJSON<T>(path: string, revalidate = 0): Promise<T> {
  const res = await fetch(`${API_URL}${path}`, {
    cache: revalidate ? "force-cache" : "no-store",
    next: revalidate ? { revalidate } : undefined,
  });
  return read<T>(res);
}

export async function postJSON<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${API_URL}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  return read<T>(res);
}

export const fmt = {
  n0: (v: number) => new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 }).format(v),
  n1: (v: number) => new Intl.NumberFormat("en-US", { maximumFractionDigits: 1 }).format(v),
  usd: (v: number) =>
    new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(v),
  usd2: (v: number) =>
    new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(v),
  pct: (v: number, digits = 1) => `${v > 0 ? "+" : ""}${v.toFixed(digits)}%`,
  day: (iso: string) =>
    new Date(`${iso}T12:00:00Z`).toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" }),
  shortDay: (iso: string) =>
    new Date(`${iso}T12:00:00Z`).toLocaleDateString("en-US", { month: "short", day: "numeric" }),
};
