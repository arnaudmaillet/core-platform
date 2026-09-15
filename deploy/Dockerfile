# syntax=docker/dockerfile:1
#
# Multi-stage build for any fleet binary. Select the binary with `--build-arg BIN`:
#
#   docker build -f deploy/Dockerfile --build-arg BIN=chat-server -t <repo>/chat-server:<tag> .
#   docker build -f deploy/Dockerfile --build-arg BIN=migrator    -t <repo>/migrator:<tag>    .
#
# One Dockerfile, one image per binary (matching the per-binary ECR repos).
#
# ── Two routes into the same `runtime` stage ──────────────────────────────────
#   • Local / docker-compose (`docker build --build-arg BIN=…`): the full path
#     below — cargo-chef cooks the third-party graph into a cached layer, then
#     `builder` compiles the workspace + the selected binary.
#   • CI (.github/workflows/fleet-images-deploy.yml + deploy/docker-bake.hcl):
#     the runner compiles ALL fleet binaries in one `cargo build` per arch,
#     INSIDE the `toolchain` stage below (`docker build --target toolchain`, then
#     `docker run` with the checkout and the cargo cache bind-mounted), then bakes
#     only the `runtime` stage per binary, overriding `builder` with a local
#     directory that holds the prebuilt binary at /usr/local/bin/app (BuildKit
#     named context: `--build-context builder=dist/<bin>`). chef/planner/builder
#     never run in CI.
#   Keep `runtime` the single definition of what ships — base image, packages,
#   the media ffmpeg conditional, the non-root user — so both routes agree.
#
# ── glibc invariant (why CI compiles inside `toolchain`, not on the runner) ───
# A binary links against the glibc of the machine that BUILT it and needs one at
# least that new at RUNTIME. `toolchain` and `runtime` are both Debian bookworm
# (glibc 2.36). Compiling on the GitHub runner itself (Ubuntu 24.04, glibc 2.39)
# produced binaries that died at start with "GLIBC_2.38 not found" — found live
# on the 2026-09-15 staging cycle: librdkafka's cmake build picks up strlcpy
# (glibc >= 2.38) when the build host has it. The workflow also gates on this:
# the highest GLIBC_x.y symbol a binary references must not exceed the runtime
# base's glibc. Bump `toolchain` and `runtime` bases together.
#
# ── Why cargo-chef on the local route ─────────────────────────────────────────
# A plain `COPY . .` + `cargo build -p $BIN` recompiles the entire third-party
# graph (tonic, rdkafka→librdkafka-from-source, scylla, sqlx, rusty-s3 …) on any
# source edit. cargo-chef splits it into three cache domains so an edit
# invalidates only the smallest possible slice:
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
# Caching is layer-based (no `target` cache mount) so the cook layer is reused
# across compose rebuilds on one machine. CI deliberately keeps NO registry-side
# build cache: the previous per-(bin,arch) BuildKit cache on ECR cost 83.55 USD
# of DataTransfer-Out in July 2026 (~40 GB and ~4 USD per push to develop).

# ── Toolchain base (shared by the local chef route and by CI) ─────────────────
# Exact patch pinned in lockstep with rust-toolchain.toml (see its bump
# procedure): the toolchain file reaches this stage via the image's rustup, so a
# mismatched tag makes every build download the pinned toolchain first.
# protoc: the `*-api` crates' build.rs run tonic-prost-build. libprotobuf-dev ships
# the well-known types (/usr/include/google/protobuf/*.proto) that several .proto
# files import (e.g. google/protobuf/timestamp.proto) — without it protoc fails
# "File not found". cmake + zlib: rdkafka builds librdkafka (bundled) from source.
FROM rust:1.96.0-bookworm AS toolchain
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler libprotobuf-dev cmake zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*
# Tell prost-build where the well-known types live so it adds them to protoc's
# include path (the *-api build.rs only pass their own ../proto/ as an include).
ENV PROTOC_INCLUDE=/usr/include
WORKDIR /app

# ── Chef base ─────────────────────────────────────────────────────────────────
FROM toolchain AS chef
RUN cargo install cargo-chef --locked

# ── Planner: distill the dependency recipe (source-only, cheap) ────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Builder ───────────────────────────────────────────────────────────────────
FROM chef AS builder

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
# Same Debian release as `toolchain` (glibc invariant above).
FROM debian:bookworm-slim AS runtime

# ca-certificates: OTLP/TLS egress. zlib1g: librdkafka. Run as non-root.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates zlib1g \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 -g nogroup app

# The media binaries shell out to ffmpeg/ffprobe: media-server validates uploaded
# video at finalize (ffprobe), media-worker transcodes it to the HLS ladder
# (ffmpeg). Installed only for these images so the other ~15 fleet images stay
# lean. `BIN` is re-declared here to be visible in the runtime stage.
ARG BIN=chat-server
RUN if [ "$BIN" = "media-server" ] || [ "$BIN" = "media-worker" ]; then \
        apt-get update && apt-get install -y --no-install-recommends ffmpeg \
        && rm -rf /var/lib/apt/lists/*; \
    fi

COPY --from=builder /usr/local/bin/app /usr/local/bin/app

USER 10001
ENTRYPOINT ["/usr/local/bin/app"]
