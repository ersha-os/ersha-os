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
    cd ersha-prime && cargo run -- {{ARGS}}

# Run ersha-dispatch service (default config: ersha-dispatch.toml)
run-dispatch *ARGS:
    cd ersha-dispatch && cargo run  -- {{ARGS}}

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

# ============================================================
# TLS & Security Recipes
# ============================================================

tls_dir          := "ersha-tls"
prime_keys       := "ersha-prime/keys"
dispatch_keys    := "ersha-dispatch/keys"
rpc_example_keys := "ersha-rpc/examples/keys"

# Generate mTLS certificates and distribute to all crates
tls-setup: tls-gen tls-dist tls-clean-tmp

# Generate the actual certificates inside the ersha-tls directory
tls-gen:
    @echo "Generating mTLS assets..."
    cd {{tls_dir}} && openssl genrsa -out root_ca.key 4096
    cd {{tls_dir}} && openssl req -x509 -new -nodes -key root_ca.key -sha256 -days 1024 -out root_ca.crt \
      -subj "/CN=MyLocalCA"
    # Server Cert
    cd {{tls_dir}} && openssl genrsa -out server_raw.key 2048
    cd {{tls_dir}} && openssl req -new -key server_raw.key -out server.csr -subj "/CN=localhost"
    cd {{tls_dir}} && openssl x509 -req -in server.csr -CA root_ca.crt -CAkey root_ca.key -CAcreateserial \
      -out server.crt -days 500 -sha256 \
      -extfile <(printf "subjectAltName=DNS:localhost,IP:127.0.0.1")
    # Client Cert
    cd {{tls_dir}} && openssl genrsa -out client_raw.key 2048
    cd {{tls_dir}} && openssl req -new -key client_raw.key -out client.csr -subj "/CN=my-client"
    cd {{tls_dir}} && openssl x509 -req -in client.csr -CA root_ca.crt -CAkey root_ca.key -CAcreateserial \
      -out client.crt -days 500 -sha256
    # Convert keys to PKCS#8
    cd {{tls_dir}} && openssl pkcs8 -topk8 -inform PEM -outform PEM -nocrypt -in server_raw.key -out server.key
    cd {{tls_dir}} && openssl pkcs8 -topk8 -inform PEM -outform PEM -nocrypt -in client_raw.key -out client.key

# Distribute keys to prime, dispatch, and rpc examples
tls-dist:
    @echo "Distributing keys to crates..."
    mkdir -p {{prime_keys}} {{dispatch_keys}} {{rpc_example_keys}}
    # Server gets server keys + root CA
    cp {{tls_dir}}/server.crt {{tls_dir}}/server.key {{tls_dir}}/root_ca.crt {{prime_keys}}/
    # Client gets client keys + root CA
    cp {{tls_dir}}/client.crt {{tls_dir}}/client.key {{tls_dir}}/root_ca.crt {{dispatch_keys}}/
    # RPC examples get everything to facilitate both client/server examples
    cp {{tls_dir}}/server.crt {{tls_dir}}/server.key {{tls_dir}}/client.crt {{tls_dir}}/client.key {{tls_dir}}/root_ca.crt {{rpc_example_keys}}/

# Clean intermediate files inside the tls directory
tls-clean-tmp:
    @echo "Cleaning temporary signing files..."
    cd {{tls_dir}} && rm -f *.csr server_raw.key client_raw.key root_ca.srl

# Wipe all generated keys from the entire workspace for a fresh start
tls-wipe:
    @echo "Wiping all TLS assets from workspace..."
    rm -rf {{prime_keys}} {{dispatch_keys}} {{rpc_example_keys}}
    cd {{tls_dir}} && rm -f *.crt *.key *.csr *.srl

# ============================================================
# Frontend Deployment (to Axum)
# ============================================================

ersha_registry_dir := "ersha-registry"
prime_public := "ersha-prime/public"

# Build UI and move assets to ersha-prime/public for Axum to serve
ersha-registry-deploy:
    @echo "Building ersha-registry..."
    cd {{ersha_registry_dir}} && npm run build

    @echo "Preparing ersha-prime public directory..."
    mkdir -p {{prime_public}}

    # Clean old assets to prevent staleness
    rm -rf {{prime_public}}/*

    @echo "Moving built files to {{prime_public}}..."
    cp -r {{ersha_registry_dir}}/dist/* {{prime_public}}/

    @echo "Done! Axum can now serve the UI from {{prime_public}}"

# Run ersha registry for dev
ersha-registry-run:
  cd {{ersha_registry_dir}} && npm run dev
