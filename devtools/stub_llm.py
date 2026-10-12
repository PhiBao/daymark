"""TEST DOUBLE — NOT used in the demo, never part of any claimed result.

A minimal OpenAI-compatible server that fabricates grounded copy from the FACTS
JSON in the prompt. Its only purpose is to exercise the backend's generation +
grounding pipeline on a laptop before the real Gemma-vLLM-on-AMD endpoint exists.

Run:  python3 devtools/stub_llm.py [port]     (default 8000)
Env:  STUB_INVENT=1  → deliberately emit an invented number once, to test the
                       grounding rejection + retry path.
"""

from __future__ import annotations

import json
import os
import re
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

INVENT_ONCE = os.environ.get("STUB_INVENT") == "1"
_invented = False


def facts_from_prompt(content: str) -> dict:
    m = re.search(r"FACTS:\s*(\{.*\})\s*$", content, re.S)
    if not m:
        return {}
    try:
        return json.loads(m.group(1))
    except json.JSONDecodeError:
        return {}


def build_narrative(facts: dict) -> dict:
    t = facts.get("totals", {})
    promos = facts.get("promos", [])
    creative = []
    for p in promos[:2]:
        creative.append(
            {
                "item": p.get("item", 0),
                "headline": f"{p.get('family', 'Fresh')} · today only",
                "body": (
                    f"Measured lift {round(p.get('lift_pct', 0))}% on this category. "
                    f"Shelf stock covers {round(p.get('cover_days', 0))} days."
                ),
            }
        )
    narrative = {
        "headline": f"{t.get('order_lines', 0)} orders, {t.get('markdown_lines', 0)} markdowns due",
        "dek": (
            f"{round(t.get('onhand_units', 0))} units on hand, {round(t.get('at_risk_units', 0))} at risk of expiry."
        ),
        "orders_summary": (
            f"Place {t.get('order_lines', 0)} order lines totalling {t.get('order_units', 0)} units. "
            f"Expected sales today: {round(t.get('expected_sales_today', 0))} units."
        ),
        "markdowns_summary": (
            f"{t.get('markdown_lines', 0)} lines are inside their last two days, "
            f"covering {round(t.get('at_risk_units', 0))} units at risk. "
            f"Recovered value: {round(t.get('recovered_usd', 0))} dollars."
        ),
        "promos_summary": (
            f"{t.get('promo_lines', 0)} promo candidates, expected "
            f"{round(t.get('expected_incremental_units', 0))} incremental units."
        ),
        "watchlist_summary": f"{len(facts.get('watchlist', []))} items flagged.",
        "promo_creative": creative,
    }
    return narrative


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):  # quieter
        pass

    def do_GET(self):
        if self.path.rstrip("/") in ("/v1/models", "/models"):
            body = json.dumps({"data": [{"id": "stub-model", "object": "model"}], "object": "list"}).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        global _invented
        if self.path.rstrip("/") != "/v1/chat/completions":
            self.send_response(404)
            self.end_headers()
            return
        length = int(self.headers.get("content-length", "0"))
        req = json.loads(self.rfile.read(length) or b"{}")
        user = next((m.get("content", "") for m in req.get("messages", []) if m.get("role") == "user"), "")
        facts = facts_from_prompt(user)
        narrative = build_narrative(facts)
        if INVENT_ONCE and not _invented:
            _invented = True
            narrative["orders_summary"] += " Save 42713 dollars this week."
            narrative["headline"] = "Test headline"
        content = json.dumps(narrative)
        resp = {
            "id": "stub-1",
            "object": "chat.completion",
            "model": req.get("model", "stub-model"),
            "choices": [{"index": 0, "message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
            "usage": {
                "prompt_tokens": max(1, len(user) // 4),
                "completion_tokens": max(1, len(content) // 4),
                "total_tokens": (len(user) + len(content)) // 4,
            },
        }
        body = json.dumps(resp).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
    print(f"stub LLM (TEST DOUBLE) listening on :{port}  invent_once={INVENT_ONCE}")
    HTTPServer(("127.0.0.1", port), Handler).serve_forever()
