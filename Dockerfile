# ---- Build stage ----

FROM rust:1.88 AS builder

WORKDIR /app

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev clang llvm-dev libclang-dev cmake curl && \
    rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -r src

# Copy actual source code and build
COPY src src
RUN cargo build --release

# ---- Runtime stage ----

FROM debian:bookworm-slim

# Install minimal system deps
RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/rpc-auth-proxy /usr/local/bin/app

CMD ["app"]
