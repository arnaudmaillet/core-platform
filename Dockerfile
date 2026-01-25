FROM debian:12-slim
# Installation des libs minimales pour Rust/Kafka
RUN apt-get update && apt-get install -y libtinfo5 ca-certificates && rm -rf /var/lib/apt/lists/*
# On copie le binaire compilé par la CI
COPY bazel-bin/backend/gateway/graphql-bff/graphql_bff /usr/local/bin/app
CMD ["/usr/local/bin/app"]