.PHONY: help data engine web api dev test build docker smoke replay clean

help:
	@echo "Daymark — target reference"
	@echo "  make data     download Favorita source files (~900MB, once)"
	@echo "  make engine   curate + train forecaster + export serving artifacts"
	@echo "  make api      run the Rust API (expects engine artifacts)"
	@echo "  make web      run the Next.js frontend (dev)"
	@echo "  make dev      api + web (dev)"
	@echo "  make test     rust tests + web typecheck/build"
	@echo "  make smoke    end-to-end API smoke test"
	@echo "  make replay   print the replay proof for store 2 (deterministic)"

data:
	cd engine && uv run python -m daymark.fetch

engine:
	cd engine && uv run python -m daymark.curate \
		&& uv run python -m daymark.train \
		&& uv run python -m daymark.export_serving

api:
	cd backend && cargo run

web:
	cd web && pnpm dev

dev:
	@echo "start `make api` in one terminal and `make web` in another"

test:
	cd backend && cargo test
	cd web && pnpm build

smoke:
	bash scripts/smoke.sh

replay:
	curl -s "http://localhost:8787/api/replay?store=2" | jq '{window, deltas: .deltas}'

clean:
	rm -rf engine/artifacts/*.parquet engine/artifacts/*.json engine/artifacts/model backend/state
