# ersha-dispatch

A regional ingestion gateway that sits between [edge devices](../ersha-edge) and the central [ersha-prime](../ersha-prime) server. It collects sensor readings and device status updates from edge devices, buffers them locally, and batch-uploads them to ersha-prime over mTLS.

## What it does

- Receives sensor readings (soil moisture, temperature, humidity, rainfall) and device statuses from edge devices
- Buffers data locally using in-memory or SQLite storage
- Periodically batch-uploads buffered data to ersha-prime via [ersha-rpc](../ersha-rpc)
- Generates alerts for critical conditions (low battery, sensor failures)
- Tracks device connections and disconnections
- Exposes an HTTP `/health` endpoint for liveness checks

## Usage

```
cargo run -p ersha-dispatch -- --config ./ersha-dispatch.toml
```

If `--config` (or `-c`) is omitted, it defaults to `ersha-dispatch.toml` in the working directory.

Set `RUST_LOG` to control log verbosity (default: `tracing=info,ersha_dispatch=info`).

## Configuration

The config file is TOML. A sample is available at [ersha-dispatch.toml](ersha-dispatch.toml).

### `[dispatcher]`

Identifies this dispatcher instance.

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | ULID that uniquely identifies this dispatcher |
| `location` | integer | H3 resolution-10 cell index representing the geographic area this dispatcher covers |

### `[server]`

| Field | Type | Description |
|-------|------|-------------|
| `http_addr` | string | Address for the HTTP health-check server (e.g. `"0.0.0.0:8081"`) |

### `[storage]`

Controls how sensor data is buffered before upload. The `type` field selects the backend.

**In-memory** (no persistence, suitable for testing):

```toml
[storage]
type = "memory"
```

**SQLite** (persistent, survives restarts):

```toml
[storage]
type = "sqlite"
path = "dispatch.db"
```

### `[prime]`

Connection settings for the ersha-prime backend.

| Field | Type | Description |
|-------|------|-------------|
| `rpc_addr` | string | Address of the ersha-prime RPC server (e.g. `"127.0.0.1:9000"`) |
| `upload_interval_secs` | integer | Seconds between batch uploads |

### `[edge]`

Configures how the dispatcher receives data from edge devices. The `type` field selects the source.

**Mock** (generates synthetic sensor data for testing):

```toml
[edge]
type = "mock"
reading_interval_secs = 5
status_interval_secs = 30
device_count = 100
```

| Field | Type | Description |
|-------|------|-------------|
| `reading_interval_secs` | integer | Seconds between generated sensor readings |
| `status_interval_secs` | integer | Seconds between generated device status updates |
| `device_count` | integer | Number of simulated devices |

**TCP** (listens for real edge devices):

```toml
[edge]
type = "tcp"
addr = "0.0.0.0:8000"
```

| Field | Type | Description |
|-------|------|-------------|
| `addr` | string | TCP address to listen on for incoming edge device connections |

### `[tls]`

Mutual TLS credentials used to authenticate with ersha-prime.

| Field | Type | Description |
|-------|------|-------------|
| `cert` | string | Path to the client certificate |
| `key` | string | Path to the client private key |
| `root_ca` | string | Path to the root CA certificate for verifying the server |
| `domain` | string | Expected server domain name for certificate validation |
