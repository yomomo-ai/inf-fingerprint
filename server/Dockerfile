FROM rust:1.85-slim AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY client/ client/
COPY server/ server/

RUN cargo build --release -p inf-fingerprint-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 1000 -m -s /bin/false app

WORKDIR /app
COPY --from=builder /build/target/release/inf-fingerprint-server /usr/local/bin/inf-fingerprint-server

USER app
EXPOSE 28091
ENV INF_FP_CONFIG=/app/config.toml
CMD ["/usr/local/bin/inf-fingerprint-server"]
