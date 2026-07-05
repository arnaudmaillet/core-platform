# syntax=docker/dockerfile:1
#
# Multi-stage build for any fleet binary, with `cargo-chef` dependency caching.
# Select the binary with `--build-arg BIN`:
#
#   docker build -f deploy/Dockerfile --build-arg BIN=chat-server -t <repo>/chat-server:<tag> .
#   docker build -f deploy/Dockerfile --build-arg BIN=migrator    -t <repo>/migrator:<tag>    .
#
# One Dockerfile, one image per binary (matching the per-binary ECR repos).
#
# ── Why cargo-chef ────────────────────────────────────────────────────────────
# The previous layout did `COPY . .` then `cargo build -p $BIN`, so *any* source
# edit invalidated the build layer and recompiled the entire third-party graph
# (tonic, rdkafka→librdkafka-from-source, scylla, sqlx, rusty-s3 …) on every CI
# run. cargo-chef splits that graph into a stable, separately-cached layer.
#
# The build is partitioned into three cache domains so an edit invalidates only
# the smallest possible slice:
#
#   1. recipe  — `cargo chef prepare` distills Cargo.toml/Cargo.lock into
#                recipe.json. Changes ONLY when the dependency graph changes.
#   2. cook    — `cargo chef cook` compiles the *third-party* graph from the
#                recipe. This is the expensive layer (librdkafka, tonic deps).
#                Cached until recipe.json changes — NOT busted by a .rs/.proto edit.
#   3. build   — compiles workspace crates + the selected binary. cargo-chef
#                stubs workspace members during `cook`, so OUR crates — including
#                the `*-api` contract crates whose build.rs runs tonic-prost-build
#                (protobuf codegen) — are (re)compiled HERE, not in `cook`. That is
#                the protobuf cache-busting isolation: editing a .proto rebuilds
#                only this layer, never the third-party cook layer.
#
# Caching is layer-based (no `target` cache mount) so it is portable and
# deterministic across ephemeral CI runners via a registry-backed BuildKit cache:
#
#   docker buildx build -f deploy/Dockerfile --build-arg BIN=$BIN \
#     --cache-to   type=registry,ref=<ecr>/buildcache:$BIN,mode=max \
#     --cache-from type=registry,ref=<ecr>/buildcache:$BIN \
#     -t <ecr>/core-platform-$BIN:$TAG --push .

# ── Chef base ─────────────────────────────────────────────────────────────────
# Minor pinned in lockstep with rust-toolchain.toml (see its bump procedure) —
# the floating `rust:1` tag is the same drift class that flapped the CI clippy
# gate on launch day. NB: the toolchain file also reaches this stage via the
# image's rustup, so a mismatched tag would trigger a redundant download.
FROM rust:1.96-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

# ── Planner: distill the dependency recipe (source-only, cheap) ────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Builder ───────────────────────────────────────────────────────────────────
FROM chef AS builder

# protoc: the `*-api` crates' build.rs run tonic-prost-build. libprotobuf-dev ships
# the well-known types (/usr/include/google/protobuf/*.proto) that several .proto
# files import (e.g. google/protobuf/timestamp.proto) — without it protoc fails
# "File not found". cmake + zlib: rdkafka builds librdkafka (bundled) from source.
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler libprotobuf-dev cmake zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

# Tell prost-build where the well-known types live so it adds them to protoc's
# include path (the *-api build.rs only pass their own ../proto/ as an include).
ENV PROTOC_INCLUDE=/usr/include

# 1+2: cook the third-party graph. Baked into THIS layer (no target cache mount),
# so it is reused via Docker/BuildKit layer cache until recipe.json changes. The
# cargo registry IS cache-mounted — it only speeds the crate *download*, it holds
# no compiled output, so it never defeats the layer cache.
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo chef cook --release --recipe-path recipe.json

# 3: build workspace crates + the selected binary. Only this layer (and the COPY
# above it) is invalidated by a .rs/.proto edit; the cooked deps above survive.
COPY . .
ARG BIN=chat-server
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release --locked -p "${BIN}" \
    && cp "/app/target/release/${BIN}" /usr/local/bin/app

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# ca-certificates: OTLP/TLS egress. zlib1g: librdkafka. Run as non-root.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates zlib1g \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 -g nogroup app

COPY --from=builder /usr/local/bin/app /usr/local/bin/app

USER 10001
ENTRYPOINT ["/usr/local/bin/app"]
