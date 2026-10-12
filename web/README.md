# Daymark web

The morning edition. Next.js (App Router) + TypeScript, custom newsprint design
system — no component library. Pages: Brief (`/`), Replay (`/replay`),
Ledger (`/ledger`), Trust (`/trust`).

```bash
pnpm install
NEXT_PUBLIC_API_URL=http://localhost:8787 pnpm dev   # API must be running
pnpm build                                           # typecheck + production build
```

Environment: `NEXT_PUBLIC_API_URL` (browser-reachable Daymark API).
