#!/usr/bin/env bash
# End-to-end API smoke test — the core product loop must work:
#   facts -> generate (grounded) -> decision -> ledger chain verification -> replay
set -euo pipefail

BASE="${DAYMARK_API:-http://localhost:8787}"
STORE="${STORE:-2}"
DATE="${DATE:-2017-08-14}"
fail=0

check() { # name, condition
  if [ "$2" = "1" ]; then echo "  ok   $1"; else echo "  FAIL $1"; fail=1; fi
}

echo "smoke: $BASE store=$STORE date=$DATE"

H=$(curl -fsS "$BASE/api/health")
check "health" "$(echo "$H" | jq -r 'if .ok then 1 else 0 end')"

F=$(curl -fsS "$BASE/api/brief/$STORE?date=$DATE")
check "facts: orders present" "$(echo "$F" | jq -r 'if (.facts.orders|length) > 0 then 1 else 0 end')"
check "facts: totals consistent" "$(echo "$F" | jq -r '
  .facts as $f |
  if ($f.totals.order_units == ([$f.orders[].suggest_qty] | add // 0)) then 1 else 0 end')"

G=$(curl -fsS -X POST "$BASE/api/brief/$STORE/generate?date=$DATE&refresh=true")
check "generate: grounded" "$(echo "$G" | jq -r 'if .grounding.ok then 1 else 0 end')"
check "generate: headline present" "$(echo "$G" | jq -r 'if (.narrative.headline|length) > 3 then 1 else 0 end')"

ITEM=$(echo "$F" | jq -r '.facts.orders[0].item')
D=$(curl -fsS -X POST "$BASE/api/decisions" -H 'content-type: application/json' \
  -d "{\"store\":$STORE,\"brief_date\":\"$DATE\",\"item\":$ITEM,\"action\":\"approve\",\"qty\":1,\"reason\":{\"source\":\"smoke\"}}")
check "decision: hash returned" "$(echo "$D" | jq -r 'if (.entry.hash|length) == 64 then 1 else 0 end')"

L=$(curl -fsS "$BASE/api/ledger?store=$STORE")
check "ledger: chain verifies" "$(echo "$L" | jq -r 'if .chain_ok then 1 else 0 end')"

R=$(curl -fsS "$BASE/api/replay?store=$STORE")
check "replay: three policies" "$(echo "$R" | jq -r 'if (.policies|length) == 3 then 1 else 0 end')"
check "replay: deterministic accounting" "$(echo "$R" | jq -r '
  .policies as $p |
  if ([$p[] | (.totals.sales_units + .totals.lost_units - .totals.demand_units) | fabs] | max) < 0.5 then 1 else 0 end')"
check "replay: daymark cuts waste" "$(echo "$R" | jq -r 'if .deltas.daymark_vs_spreadsheet.waste_units_delta_pct < 0 then 1 else 0 end')"

if [ "$fail" = "1" ]; then echo "SMOKE FAILED"; exit 1; fi
echo "smoke: all checks passed"
