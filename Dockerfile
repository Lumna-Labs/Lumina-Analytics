# Builds both lumina-api and lumina-ingest from one image; docker-compose
# selects which binary to run per service via `command`.
FROM rust:1-slim-bookworm AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release --bins

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=builder /build/target/release/lumina-api /usr/local/bin/lumina-api
COPY --from=builder /build/target/release/lumina-ingest /usr/local/bin/lumina-ingest

# Overridden by docker-compose's `command:` per service.
ENTRYPOINT ["lumina-api"]
