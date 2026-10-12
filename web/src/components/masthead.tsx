import Link from "next/link";

export function Wordmark({ size = 22 }: { size?: number }) {
  return (
    <span className="inline-flex items-center gap-2">
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        aria-hidden="true"
        className="shrink-0"
      >
        <rect x="1" y="1" width="22" height="22" rx="3" fill="var(--color-ink)" />
        <path d="M6 17V7h3.4c3.2 0 5.1 1.9 5.1 5s-1.9 5-5.1 5H6z" fill="var(--color-paper)" />
        <circle cx="18.6" cy="16.9" r="1.6" fill="var(--color-green)" />
      </svg>
      <span className="display text-[1.05rem] font-semibold tracking-tight">Daymark</span>
    </span>
  );
}

const NAV = [
  { href: "/", label: "Brief" },
  { href: "/replay", label: "Replay" },
  { href: "/ledger", label: "Ledger" },
  { href: "/trust", label: "Trust" },
];

export function Masthead() {
  return (
    <header className="rule-double mb-6 border-b border-[var(--color-rule)] bg-[var(--color-paper)]">
      <div className="mx-auto flex max-w-[1180px] flex-wrap items-center justify-between gap-y-2 px-4 py-3 sm:px-6">
        <Link href="/" aria-label="Daymark home" className="hover:opacity-80">
          <Wordmark />
        </Link>
        <nav className="flex items-center gap-1 text-[12px] tracking-[0.12em] uppercase">
          {NAV.map((n) => (
            <Link
              key={n.href}
              href={n.href}
              className="rounded-[2px] px-2 py-1.5 text-[var(--color-ink-2)] transition-colors hover:bg-[var(--color-ink)] hover:text-[var(--color-paper)] sm:px-3"
            >
              {n.label}
            </Link>
          ))}
        </nav>
      </div>
    </header>
  );
}
