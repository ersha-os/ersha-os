# Stage 1: Chef - install cargo-chef
FROM rust:1.92-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# Stage 2: Planner - prepare recipe.json for dependency caching
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: UI Builder - build the Elm/Vite registry frontend
FROM --platform=linux/amd64 node:22 AS ui-builder
WORKDIR /app
COPY ersha-registry/package.json ersha-registry/package-lock.json ./
RUN npm ci
COPY ersha-registry/ .
RUN npm run build

# Stage 4: Builder - cook dependencies then build all binaries
FROM chef AS builder
RUN apt-get update && apt-get install -y libsqlite3-dev pkg-config && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
COPY --from=ui-builder /app/dist ersha-registry/dist
RUN cargo build --release -p ersha-prime -p ersha-dispatch -p ersha-dispatchers-harness

# Stage 5: Runtime base - slim Debian with shared runtime deps
FROM debian:bookworm-slim AS runtime-base
RUN apt-get update && apt-get install -y ca-certificates libsqlite3-0 curl && rm -rf /var/lib/apt/lists/*

# Stage 6: ersha-prime
FROM runtime-base AS ersha-prime
COPY --from=builder /app/target/release/ersha-prime /usr/local/bin/ersha-prime
EXPOSE 9000 8080
ENTRYPOINT ["ersha-prime"]

# Stage 7: ersha-harness (includes both harness and dispatch binaries)
FROM runtime-base AS ersha-harness
COPY --from=builder /app/target/release/ersha-dispatchers-harness /usr/local/bin/ersha-dispatchers-harness
COPY --from=builder /app/target/release/ersha-dispatch /usr/local/bin/ersha-dispatch
ENTRYPOINT ["ersha-dispatchers-harness"]
