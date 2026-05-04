FROM rust:1.87 AS builder

ARG SERVICE
ARG BIN

WORKDIR /app
COPY . .
RUN cargo build --release -p ${SERVICE} && cp target/release/${BIN} /tmp/service-binary

FROM debian:bookworm-slim

WORKDIR /app
ENV SERVICE_HOST=0.0.0.0
COPY --from=builder /tmp/service-binary /app/service
EXPOSE 8080 8081 8082 8083 8084 8085
CMD ["/app/service"]
