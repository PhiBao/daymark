#!/usr/bin/env bash
# Serve Gemma with vLLM on an AMD Instinct GPU (ROCm).
#
#   MODEL=google/gemma-3-27b-it bash serve_gemma.sh
#
# Prefers the official ROCm vLLM container; falls back to a Python venv.
set -euo pipefail

MODEL="${MODEL:-google/gemma-3-27b-it}"
PORT="${PORT:-8000}"
API_KEY="${API_KEY:-daymark}"
GPU_UTIL="${GPU_UTIL:-0.90}"
MAX_LEN="${MAX_LEN:-8192}"

echo "== GPU =="
(rocm-smi || amd-smi) 2>/dev/null | head -20 || echo "no rocm-smi/amd-smi found (are you on an AMD GPU host?)"

if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  echo "== serving via vLLM ROCm container =="
  docker rm -f daymark-vllm >/dev/null 2>&1 || true
  exec docker run --rm --name daymark-vllm \
    --device=/dev/kfd --device=/dev/dri \
    --group-add video --ipc=host --shm-size 16G \
    -e VLLM_ROCM_USE_AITER=1 \
    -v "${HOME}/.cache/huggingface:/root/.cache/huggingface" \
    -p "${PORT}:8000" \
    rocm/vllm:latest \
    vllm serve "${MODEL}" \
      --host 0.0.0.0 --port 8000 \
      --api-key "${API_KEY}" \
      --max-model-len "${MAX_LEN}" \
      --gpu-memory-utilization "${GPU_UTIL}"
fi

echo "== docker not available; using a Python venv =="
python3 -m venv .venv-vllm
. .venv-vllm/bin/activate
pip install --upgrade pip
pip install vllm
exec vllm serve "${MODEL}" \
  --host 0.0.0.0 --port "${PORT}" \
  --api-key "${API_KEY}" \
  --max-model-len "${MAX_LEN}" \
  --gpu-memory-utilization "${GPU_UTIL}"
