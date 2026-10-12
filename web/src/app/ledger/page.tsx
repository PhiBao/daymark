import Link from "next/link";
import { LedgerEntry, fmt, getJSON } from "@/lib/api";

export const instant = false;


export default async function Page(props: { searchParams: Promise<{ store?: string }> }) {
  const sp = await props.searchParams;
  const store = Number(sp.store ?? "2");

  let entries: LedgerEntry[] = [];
  let chainOk = true;
  let error: string | null = null;
  try {
    const resp = await getJSON<{ entries: LedgerEntry[]; chain_ok: boolean; count: number }>(
      `/api/ledger?store=${store}`,
    );
    entries = resp.entries;
    chainOk = resp.chain_ok;
  } catch (e) {
    error = String(e);
  }

  return (
    <div>
      <div className="mt-6 border-b border-[var(--color-ink)] pb-2 text-[11px] tracking-[0.18em] uppercase">
        Ledger · every decision signed into the chain
      </div>

      <div className="mt-6 grid gap-8 lg:grid-cols-[1.7fr_1fr]">
        <div>
          <h1 className="display text-3xl leading-[1.08] font-semibold sm:text-[2.4rem]">
            What was approved, and when.
          </h1>
          <p className="mt-3 max-w-[68ch] text-[14px] text-[var(--color-ink-2)]">
            Each decision stores its reason and the hash of the entry before it. Change any row afterwards and
            the chain no longer verifies — the audit trail is tamper-evident, not just tamper-discouraged.
          </p>
        </div>
        <div className="content-start self-start border-l border-[var(--color-rule)] pl-5">
          <div className="text-[10px] tracking-[0.16em] text-[var(--color-muted)] uppercase">Chain status</div>
          <div className={`display mt-1 text-[1.4rem] font-semibold ${chainOk ? "text-[var(--color-green)]" : "text-[var(--color-red)]"}`}>
            {chainOk ? "verified" : "broken"}
          </div>
          <div className="mono mt-1 text-[12px] text-[var(--color-muted)]">
            {entries.length} entries · store {store}
          </div>
        </div>
      </div>

      {error ? (
        <div className="rule-top mt-6 border border-[var(--color-red)] bg-[var(--color-red-soft)] p-3 text-[13px] text-[var(--color-red)]">
          {error}
        </div>
      ) : null}

      {entries.length === 0 ? (
        <div className="rule-top mt-8 border border-[var(--color-rule)] bg-[var(--color-paper-2)] p-8 text-center">
          <div className="display text-xl">Nothing on the page yet.</div>
          <p className="mt-2 text-[13.5px] text-[var(--color-muted)]">
            Approve a decision on the{" "}
            <Link href="/" className="underline decoration-[var(--color-rule)] underline-offset-2">
              brief
            </Link>{" "}
            and it appears here, signed.
          </p>
        </div>
      ) : (
        <div className="rule-top mt-8 overflow-x-auto pt-4">
          <table className="w-full min-w-[720px] border-collapse text-[13px]">
            <thead>
              <tr className="text-left text-[10px] tracking-[0.14em] text-[var(--color-muted)] uppercase">
                <th className="py-2 pr-4 font-normal">#</th>
                <th className="py-2 pr-4 font-normal">When</th>
                <th className="py-2 pr-4 font-normal">Brief</th>
                <th className="py-2 pr-4 font-normal">Item</th>
                <th className="py-2 pr-4 font-normal">Action</th>
                <th className="py-2 pr-4 text-right font-normal">Units</th>
                <th className="py-2 pr-4 font-normal">Hash</th>
                <th className="py-2 font-normal">Chain</th>
              </tr>
            </thead>
            <tbody className="mono">
              {entries
                .slice()
                .reverse()
                .map((e) => (
                  <tr key={e.id} className="border-b border-[var(--color-rule-2)]">
                    <td className="py-2 pr-4">{e.id}</td>
                    <td className="py-2 pr-4">{new Date(e.ts).toISOString().slice(0, 16).replace("T", " ")}</td>
                    <td className="py-2 pr-4">{e.brief_date}</td>
                    <td className="py-2 pr-4">{e.item ?? "—"}</td>
                    <td className="py-2 pr-4">{e.action}</td>
                    <td className="py-2 pr-4 text-right">{e.qty != null ? fmt.n0(e.qty) : "—"}</td>
                    <td className="py-2 pr-4 text-[11px]">{e.hash.slice(0, 12)}…</td>
                    <td className="py-2">
                      <span className={e.chain_ok ? "text-[var(--color-green)]" : "text-[var(--color-red)]"}>
                        {e.chain_ok ? "ok" : "broken"}
                      </span>
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
