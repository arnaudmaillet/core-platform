# syntax=docker/dockerfile:1
#
# Multi-stage build for any fleet binary. Select the binary with `--build-arg BIN`:
#
#   docker build -f deploy/Dockerfile --build-arg BIN=chat-server -t <repo>/chat-server:<tag> .
#   docker build -f deploy/Dockerfile --build-arg BIN=migrator    -t <repo>/migrator:<tag>    .
#
# One Dockerfile, one image per binary (matching the per-binary ECR repos).

# ── Builder ──────────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS builder

# protoc: the services' build.rs run tonic-prost-build.
# cmake + zlib: rdkafka builds librdkafka (bundled) from source.
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler cmake zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

ARG BIN=chat-server
# Cache the cargo registry + target dir across builds; copy the binary out of the
# (cache-mounted) target dir in the same RUN, since the mount is gone afterwards.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked -p "${BIN}" \
    && cp "/app/target/release/${BIN}" /usr/local/bin/app

# ── Runtime ──────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# ca-certificates: OTLP/TLS egress. zlib1g: librdkafka. Run as non-root.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates zlib1g \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 -g nogroup app

COPY --from=builder /usr/local/bin/app /usr/local/bin/app

USER 10001
ENTRYPOINT ["/usr/local/bin/app"]
