"""Benchmark the AMD-hosted model endpoint that powers Daymark's language layer.

Measures what the submission asks for: the serving stack, TTFT, decode
throughput at several concurrency levels, and the derived $/1M tokens from the
posted GPU price. Writes engine/artifacts/benchmark.json (consumed by the API's
cost meter and the Trust page).

Usage (against the AMD endpoint):
    DAYMARK_LLM_BASE_URL=http://host:8000/v1 DAYMARK_LLM_API_KEY=daymark \
    DAYMARK_GPU_NAME="AMD Instinct MI300X (AMD Developer Cloud)" \
    DAYMARK_GPU_HOURLY_USD=1.99 \
    uv run python -m daymark.bench

Works against any OpenAI-compatible endpoint (e.g. the local test double) so
the harness itself is testable without a GPU.
"""

from __future__ import annotations

import json
import os
import statistics
import time
from concurrent.futures import ThreadPoolExecutor

import httpx

from daymark.config import ARTIFACTS

BASE = os.environ.get("DAYMARK_LLM_BASE_URL", "http://127.0.0.1:8000/v1")
KEY = os.environ.get("DAYMARK_LLM_API_KEY", "daymark")
MODEL = os.environ.get("DAYMARK_LLM_MODEL", "auto")
GPU_NAME = os.environ.get("DAYMARK_GPU_NAME", "AMD Instinct (AMD Developer Cloud)")
GPU_HOURLY_USD = float(os.environ.get("DAYMARK_GPU_HOURLY_USD", "1.99"))

PROMPT_TOKENS_TARGET = 2500
OUTPUT_TOKENS = 220
ROUNDS = 5
CONCURRENCY_LEVELS = [1, 4, 8]

# A realistic brief-style prompt: same shape as the product's own prompts.
FILLER = (
    "item 1234567 PRODUCE on hand 38.6 units, forecast p10 113.2 p50 156.9 p90 193.8, "
    "7-day average 142.3, yesterday 151.1, shelf life 5 days. "
)
PROMPT = (
    "You are the editor of a grocery morning brief. Summarise the following facts in "
    "three plain sentences, using only these numbers.\n\nFACTS:\n" + FILLER * 60
)


def fetch_model() -> str:
    try:
        r = httpx.get(f"{BASE}/models", headers={"Authorization": f"Bearer {KEY}"}, timeout=10)
        r.raise_for_status()
        data = r.json().get("data", [])
        return data[0].get("id", MODEL) if data else MODEL
    except Exception:
        return MODEL


def one_request(stream: bool = True) -> dict:
    payload = {
        "model": MODEL,
        "messages": [{"role": "user", "content": PROMPT}],
        "max_tokens": OUTPUT_TOKENS,
        "temperature": 0.2,
        "stream": stream,
    }
    t0 = time.perf_counter()
    ttft = None
    out_tokens = 0
    streamed = False
    usage = {}
    raw_lines: list[str] = []
    with httpx.Client(timeout=300) as client:
        with client.stream(
            "POST",
            f"{BASE}/chat/completions",
            headers={"Authorization": f"Bearer {KEY}"},
            json=payload,
        ) as resp:
            resp.raise_for_status()
            for line in resp.iter_lines():
                raw_lines.append(line)
                if not line or not line.startswith("data:"):
                    continue
                data = line[5:].strip()
                if data == "[DONE]":
                    break
                chunk = json.loads(data)
                if ttft is None:
                    ttft = time.perf_counter() - t0
                choices = chunk.get("choices") or [{}]
                delta = choices[0].get("delta") or {}
                if delta.get("content"):
                    out_tokens += 1
                    streamed = True
                if chunk.get("usage"):
                    usage = chunk["usage"]
    total = time.perf_counter() - t0
    if out_tokens == 0:
        # endpoint did not stream SSE — parse a plain JSON body instead
        try:
            body = json.loads("\n".join(raw_lines))
            content = body["choices"][0]["message"]["content"]
            out_tokens = max(1, len(content) // 4)
            usage = body.get("usage", usage)
            ttft = ttft or total
        except Exception:
            pass
    # Only real streaming gives a meaningful decode rate; non-streamed bodies
    # (like the local test double) are recorded but excluded from throughput.
    decode_window = total - (ttft or 0)
    decode_tps = out_tokens / decode_window if (streamed and out_tokens and decode_window > 0.01) else 0.0
    return {
        "ttft_s": ttft or total,
        "total_s": total,
        "output_tokens": out_tokens,
        "streamed": streamed,
        "decode_tokens_per_sec": decode_tps,
        "usage": usage,
    }


def run_level(concurrency: int) -> dict:
    with ThreadPoolExecutor(max_workers=concurrency) as ex:
        results = list(ex.map(lambda _: one_request(), range(concurrency * ROUNDS)))
    ttfts = sorted(r["ttft_s"] for r in results)
    decode = [r["decode_tokens_per_sec"] for r in results if r["streamed"] and r["decode_tokens_per_sec"] > 0]
    return {
        "concurrency": concurrency,
        "requests": len(results),
        "ttft_s_p50": statistics.median(ttfts),
        "ttft_s_p95": ttfts[min(len(ttfts) - 1, int(0.95 * len(ttfts)))],
        "decode_tokens_per_sec_mean": statistics.mean(decode) if decode else 0.0,
        "wall_seconds": sum(r["total_s"] for r in results) / concurrency,
    }


def main() -> None:
    model = fetch_model()
    print(f"endpoint {BASE} · model {model} · gpu {GPU_NAME} @ ${GPU_HOURLY_USD}/h")
    levels = []
    for c in CONCURRENCY_LEVELS:
        res = run_level(c)
        levels.append(res)
        print(f"  concurrency {c}: ttft_p50={res['ttft_s_p50']:.3f}s "
              f"decode={res['decode_tokens_per_sec_mean']:.1f} tok/s")

    best = max(levels, key=lambda r: r["decode_tokens_per_sec_mean"])
    tps = best["decode_tokens_per_sec_mean"]
    if tps > 0:
        usd_per_1m = GPU_HOURLY_USD / 3600.0 * 1_000_000.0 / tps
        note = None
    else:
        usd_per_1m = 0.0
        note = "endpoint did not report token streaming; decode rate unavailable"
    if note:
        print(f"warning: {note}")

    out = {
        "gpu": GPU_NAME,
        "gpu_hourly_usd": GPU_HOURLY_USD,
        "model": model,
        "endpoint": BASE,
        "measured_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "prompt_tokens_target": PROMPT_TOKENS_TARGET,
        "output_tokens": OUTPUT_TOKENS,
        "decode_tokens_per_sec": tps,
        "usd_per_1m_tokens": usd_per_1m,
        "levels": levels,
        "method": "streaming chat completions; TTFT measured to first token; decode rate = output tokens / (total - ttft)",
        "note": note,
    }
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    (ARTIFACTS / "benchmark.json").write_text(json.dumps(out, indent=2))
    print(f"wrote {ARTIFACTS / 'benchmark.json'} · ${usd_per_1m:.2f}/1M tokens")


if __name__ == "__main__":
    main()
