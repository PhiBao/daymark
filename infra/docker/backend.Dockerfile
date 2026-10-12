# Build the Daymark API (Rust). Artifacts are mounted at runtime.
FROM rust:1-slim-bookworm AS build
WORKDIR /app
COPY backend/Cargo.toml backend/Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release || true
COPY backend/src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/daymark-api /usr/local/bin/daymark-api
ENV DAYMARK_ROOT=/data \
    DAYMARK_DATA_DIR=/data/artifacts \
    DAYMARK_STATE_DIR=/data/state \
    PORT=8787
VOLUME ["/data"]
EXPOSE 8787
CMD ["daymark-api"]
