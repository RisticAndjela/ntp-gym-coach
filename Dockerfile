# syntax=docker/dockerfile:1.7

FROM rust:1.88 AS builder

ARG SERVICE
ARG BIN

WORKDIR /app
ENV CARGO_HTTP_TIMEOUT=600 \
    CARGO_NET_RETRY=10 \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo fetch --locked
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --locked --release -p ${SERVICE} \
    && cp target/release/${BIN} /tmp/service-binary

FROM debian:bookworm-slim

WORKDIR /app
ENV SERVICE_HOST=0.0.0.0
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /tmp/service-binary /app/service
EXPOSE 8080 8081 8082 8083 8084 8085
CMD ["/app/service"]
