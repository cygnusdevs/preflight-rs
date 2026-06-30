FROM rust:1.96-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends clang cmake build-essential pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ghostscript ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/preflight-rs /usr/local/bin/preflight-rs

ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
CMD ["preflight-rs"]
