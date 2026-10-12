# AMD Developer Cloud — serving Gemma for Daymark

The product's language layer (headline, section copy, promo creative) runs on an
**open model served with vLLM on an AMD Instinct GPU** via the AMD Developer
Cloud. The backend talks to it over an OpenAI-compatible endpoint; the cost
meter converts tokens into dollars using the GPU's hourly price and the
*measured* throughput from `engine/benchmark.json`.

## 1. Start an instance

1. Sign in to the **AMD Developer Cloud** (via the AMD AI Developer Program).
2. Launch a GPU instance — MI300X (192 GB) is ideal; MI250/MI350 work too with a
   smaller model. Jupyter/Ubuntu images with ROCm preinstalled are fine.
3. Open the port you will serve on (`8000` by default) in the instance firewall,
   or connect over SSH tunnels / a Jupyter proxy.

Check the GPU is visible:

```bash
rocm-smi          # or: amd-smi
```

## 2. Serve Gemma with vLLM

```bash
cd infra/amd
MODEL=google/gemma-3-27b-it bash serve_gemma.sh
```

The script uses Docker (`rocm/vllm`) when available and falls back to a Python
venv install. It serves on `0.0.0.0:8000` with API key `daymark` (change with
`API_KEY=`), `--max-model-len 8192` to keep KV cache small for our 4–6k-token
prompts.

Smaller GPU? Use a smaller model, e.g.:

```bash
MODEL=google/gemma-3-12b-it bash serve_gemma.sh
# or for a quick smoke test:
MODEL=google/gemma-3-4b-it bash serve_gemma.sh
```

Verify:

```bash
curl -s http://127.0.0.1:8000/v1/models -H "Authorization: Bearer daymark" | head -c 300
```

## 3. Point Daymark at it

On the machine running the backend:

```bash
export DAYMARK_LLM_BASE_URL=http://<AMD_INSTANCE_HOST>:8000/v1
export DAYMARK_LLM_MODEL=google/gemma-3-27b-it
export DAYMARK_LLM_API_KEY=daymark
export DAYMARK_GPU_NAME="AMD Instinct MI300X (AMD Developer Cloud)"
export DAYMARK_GPU_HOURLY_USD=1.99      # adjust to the posted ADC price
```

## 4. Record the benchmark (required submission evidence)

```bash
cd engine
DAYMARK_LLM_BASE_URL=http://<AMD_INSTANCE_HOST>:8000/v1 \
DAYMARK_LLM_API_KEY=daymark \
DAYMARK_GPU_NAME="AMD Instinct MI300X (AMD Developer Cloud)" \
DAYMARK_GPU_HOURLY_USD=1.99 \
uv run python -m daymark.bench
```

This writes `engine/artifacts/benchmark.json`: TTFT p50/p95, decode tokens/s at
concurrency 1/4/8, and `$/1M tokens` derived from the GPU price — which then
feeds the in-app cost meter and the Trust page.

## Notes

- The demo store's figures are computed by the Rust engine and the forecaster;
  the LLM only writes language and is **verified by the number-grounding
  checker** before display.
- If the endpoint is temporarily unavailable, the API degrades to a
  deterministic template and labels it (`source: template_fallback`) — it never
  fabricates a figure.
- Never commit API keys. Use environment variables.
