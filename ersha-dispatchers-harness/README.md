# ersha-dispatchers-harness

A test harness that spawns and manages multiple [ersha-dispatch](../ersha-dispatch) instances for load testing and integration testing. Each dispatcher is assigned a unique ID, a unique geographic location (H3 cell across Ethiopia), and its own HTTP port, then connected to a central [ersha-prime](../ersha-prime) server over mTLS.

## What it does

- Spawns a configurable number of `ersha-dispatch` processes in parallel
- Generates unique H3 resolution-10 cell locations spread across Ethiopia for each dispatcher
- Assigns each dispatcher a ULID, an HTTP port (incrementing from a base port), and mock edge devices
- Writes a temporary per-dispatcher config file and passes it to the spawned process
- Manages the lifecycle of all dispatchers until Ctrl+C is pressed
- Cleans up temporary config files on shutdown

## Usage

```
cargo run -p ersha-dispatchers-harness -- --config ./ersha-dispatchers-harness.toml
```

If `--config` (or `-c`) is omitted, it defaults to `ersha-dispatchers-harness.toml` in the working directory.

Set `RUST_LOG` to control log verbosity (default: `tracing=info,ersha_dispatchers_harness=info`).

## Configuration

The config file is TOML. A sample is available at [ersha-dispatchers-harness.toml](ersha-dispatchers-harness.toml).

### Root fields

| Field | Type | Description |
|-------|------|-------------|
| `dispatcher_count` | integer | Number of dispatcher instances to spawn |
| `devices_per_dispatcher` | integer | Number of mock edge devices per dispatcher |
| `reading_interval_secs` | integer | Seconds between generated sensor readings per device |
| `status_interval_secs` | integer | Seconds between generated device status updates per device |
| `upload_interval_secs` | integer | Seconds between batch uploads to ersha-prime |
| `base_http_port` | integer | Starting HTTP port; each dispatcher gets `base_http_port + index` |
| `ersha_dispatch_bin` | string | Path to the `ersha-dispatch` binary (absolute, relative, or looked up in `target/` and `PATH`) |
| `prime_rpc_addr` | string | Address of the ersha-prime RPC server (e.g. `"127.0.0.1:9000"`) |

### `[tls]`

Mutual TLS credentials passed to each spawned dispatcher for authenticating with ersha-prime.

| Field | Type | Description |
|-------|------|-------------|
| `cert` | string | Path to the client certificate |
| `key` | string | Path to the client private key |
| `root_ca` | string | Path to the root CA certificate for verifying the server |
| `domain` | string | Expected server domain name for certificate validation |
