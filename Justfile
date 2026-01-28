# Default recipe - list all available commands
default:
    @just --list

# ============================================================
# Build Recipes
# ============================================================

# Build all workspace members
build:
    cargo build

# Build all workspace members in release mode
build-release:
    cargo build --release

# Build ersha-prime
build-prime:
    cargo build -p ersha-prime

# Build ersha-dispatch
build-dispatch:
    cargo build -p ersha-dispatch

# Build ersha-dashboard (with SSR feature for binary)
build-dashboard:
    cargo build -p ersha-dashboard --features ssr

# Build ersha-core library
build-core:
    cargo build -p ersha-core

# Build ersha-rpc library
build-rpc:
    cargo build -p ersha-rpc

# ============================================================
# Run Recipes
# ============================================================

# Run ersha-prime server (default config: ersha-prime.toml)
run-prime *ARGS:
    cargo run -p ersha-prime -- {{ARGS}}

# Run ersha-dispatch service (default config: ersha-dispatch.toml)
run-dispatch *ARGS:
    cargo run -p ersha-dispatch -- {{ARGS}}

# Run ersha-dashboard (requires SSR feature)
run-dashboard *ARGS:
    cargo run -p ersha-dashboard --features ssr -- {{ARGS}}

# ============================================================
# Example Recipes (from ersha-rpc)
# ============================================================

# Run the RPC client example
example-client:
    cargo run -p ersha-rpc --example client

# Run the RPC server example
example-server:
    cargo run -p ersha-rpc --example server

# ============================================================
# Test Recipes
# ============================================================

# Run all tests
test:
    cargo test

# Run tests for a specific package
test-pkg PKG:
    cargo test -p {{PKG}}

# Run tests for ersha-core
test-core:
    cargo test -p ersha-core

# Run tests for ersha-rpc
test-rpc:
    cargo test -p ersha-rpc

# Run tests for ersha-prime
test-prime:
    cargo test -p ersha-prime

# Run tests for ersha-dispatch
test-dispatch:
    cargo test -p ersha-dispatch

# Run tests for ersha-dashboard
test-dashboard:
    cargo test -p ersha-dashboard

# ============================================================
# Infrastructure Recipes
# ============================================================

# Run ClickHouse server in Docker (no auth for local dev)
clickhouse:
    docker run -d --name ersha-clickhouse -p 8123:8123 -p 9000:9000 -e CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1 -e CLICKHOUSE_PASSWORD="" clickhouse/clickhouse-server

# Stop and remove ClickHouse container
clickhouse-stop:
    docker stop ersha-clickhouse && docker rm ersha-clickhouse

# ============================================================
# Development Recipes
# ============================================================

# Check all workspace members for errors
check:
    cargo check

# Run clippy lints
clippy:
    cargo clippy

# Format all code
fmt:
    cargo fmt

# Format check (don't modify files)
fmt-check:
    cargo fmt --check

# Clean build artifacts
clean:
    cargo clean

# Update dependencies
update:
    cargo update

# Generate documentation
doc:
    cargo doc --no-deps

# Open documentation in browser
doc-open:
    cargo doc --no-deps --open
