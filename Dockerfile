# Multi-stage build for open-runo-router.
#
#   docker build -t open-runo-router .
#   docker run --rm -p 8080:8080 --env-file .env open-runo-router
#
# See docker-compose.yml to run alongside PostgreSQL.

FROM rust:1.75-slim-bookworm AS builder
WORKDIR /usr/src/open-runo

# Workspace-wide manifest + all member crates are needed even though only
# open-runo-router (and its open-runo-core dependency) get compiled, because
# cargo must resolve the full [workspace] members list declared in Cargo.toml.
COPY Cargo.toml ./
COPY crates ./crates
RUN cargo build --release -p open-runo-router

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --create-home --shell /usr/sbin/nologin open-runo
USER open-runo
WORKDIR /home/open-runo

COPY --from=builder /usr/src/open-runo/target/release/open-runo-router /usr/local/bin/open-runo-router

ENV OPEN_RUNO_ENV=production
ENV OPEN_RUNO_BIND_ADDR=0.0.0.0:8080
ENV OPEN_RUNO_LOG_LEVEL=info

EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD curl -fsS http://localhost:8080/health || exit 1

ENTRYPOINT ["/usr/local/bin/open-runo-router"]
