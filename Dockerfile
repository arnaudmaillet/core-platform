#Dockerfile

# Utilisation de Debian Slim pour ARM64 (Mac M2)
FROM debian:bookworm-slim
ENV DEBIAN_FRONTEND=noninteractive

# 1. Installation des dépendances système, Java et Protobuf
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    python3 \
    git \
    clang \
    lld \
    ca-certificates \
    pkg-config \
    libssl-dev \
    procps \
    psmisc \
    openjdk-17-jdk \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# 2. Installation de Rust (officielle et persistante)
# On installe rustup dans /usr/local pour qu'il soit accessible partout
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path

# 3. Installation de Bazelisk
RUN curl -L https://github.com/bazelbuild/bazelisk/releases/download/v1.19.0/bazelisk-linux-arm64 -o /usr/local/bin/bazel && \
    chmod +x /usr/local/bin/bazel

# 4. Configuration finale
WORKDIR /app
# On s'assure que root a les droits sur le dossier cargo
RUN chmod -R 777 /usr/local/cargo

ENTRYPOINT ["bazel"]