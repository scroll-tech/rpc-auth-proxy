FROM lukemathwalker/cargo-chef:0.1.72-rust-1.90 AS chef

# ---- Planner stage ----

FROM chef AS planner
WORKDIR /app
COPY Cargo.toml Cargo.lock .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Build stage ----

FROM chef AS builder
WORKDIR /app

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev clang llvm-dev libclang-dev cmake curl && \
    rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY Cargo.toml Cargo.lock .
COPY src src
RUN cargo build --release

# ---- Runtime stage ----

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rpc-auth-proxy /usr/local/bin/app
CMD ["app"]
