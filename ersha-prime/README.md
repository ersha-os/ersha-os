# ersha-prime

The central coordination server for the ersha project. It manages [dispatchers](../ersha-dispatch) and [edge devices](../ersha-edge), accepts batch uploads of sensor readings and device statuses over mTLS, and exposes an HTTP REST API for device and dispatcher management. It also serves the [registry UI](../ersha-registry) as an embedded SPA.

## What it does

- Accepts mTLS connections from dispatchers and validates their identity and state
- Receives batch uploads of sensor readings and device statuses via [ersha-rpc](../ersha-rpc)
- Processes alerts and status reports from dispatchers
- Tracks device disconnections
- Provides an HTTP REST API for managing dispatchers and devices (create, list, suspend)
- Serves the embedded registry UI for browser-based management
- Exposes an HTTP `/health` endpoint for liveness checks
- Supports pluggable storage backends (in-memory, SQLite, ClickHouse)

## Usage

```
cargo run -p ersha-prime -- --config ./ersha-prime.toml
```

If `--config` (or `-c`) is omitted, it defaults to `ersha-prime.toml` in the working directory. If no config file is found, built-in defaults are used (in-memory registry, RPC on `0.0.0.0:9000`, HTTP on `0.0.0.0:8080`).

Set `RUST_LOG` to control log verbosity (default: `tracing=info,ersha_prime=info`).

## Configuration

The config file is TOML. A sample is available at [ersha-prime.toml](ersha-prime.toml).

### `[server]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `rpc_addr` | string | `"0.0.0.0:9000"` | Address for the RPC server that dispatchers connect to |
| `http_addr` | string | `"0.0.0.0:8080"` | Address for the HTTP API and registry UI |

### `[registry]`

Controls how dispatchers, devices, readings, and statuses are stored. The `type` field selects the backend.

**In-memory** (no persistence, suitable for development/testing):

```toml
[registry]
type = "memory"
```

**SQLite** (persistent, single-file database):

```toml
[registry]
type = "sqlite"
path = "./data.db"
```

| Field | Type | Description |
|-------|------|-------------|
| `path` | string | Path to the SQLite database file |

**ClickHouse** (distributed OLAP database, suited for production):

```toml
[registry]
type = "clickhouse"
url = "http://localhost:8123"
database = "ersha_test"
```

| Field | Type | Description |
|-------|------|-------------|
| `url` | string | ClickHouse HTTP interface URL |
| `database` | string | Database name to use |

### `[tls]`

Mutual TLS credentials used to authenticate dispatcher connections.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cert` | string | `"./keys/server.crt"` | Path to the server certificate |
| `key` | string | `"./keys/server.key"` | Path to the server private key |
| `root_ca` | string | `"./keys/root_ca.crt"` | Path to the root CA certificate for verifying client (dispatcher) certificates |
| `domain` | string | `"localhost"` | Domain name for TLS |
