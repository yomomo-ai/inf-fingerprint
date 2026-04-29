# syntax=docker/dockerfile:1.7

# Distill Cargo.toml/lock into a recipe describing the dependency graph.
FROM lukemathwalker/cargo-chef:latest-rust-latest AS planner
WORKDIR /build
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Cook dependencies from the recipe. This layer is cached as long as the
# recipe is unchanged (i.e. Cargo.toml and Cargo.lock haven't moved), so
# editing source code reruns only the final cargo build, not the ~5min deps
# compile.
FROM lukemathwalker/cargo-chef:latest-rust-latest AS builder
WORKDIR /build
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --package inf-fingerprint-server

COPY . .
RUN cargo build --release -p inf-fingerprint-server

# Distroless cc-debian13: glibc + libssl + ca-certs, no shell, no package
# manager, no busybox. ~25MB base; final image ~35MB; runs as nonroot
# (uid 65532) with no writable filesystem outside /home/nonroot.
FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=builder /build/target/release/inf-fingerprint-server /usr/local/bin/inf-fingerprint-server

EXPOSE 28091
ENV INF_FP_CONFIG=/app/config.toml
WORKDIR /app
ENTRYPOINT ["/usr/local/bin/inf-fingerprint-server"]
