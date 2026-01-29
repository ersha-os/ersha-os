use std::path::{Path, PathBuf};

use clap::Parser;
use h3o::{LatLng, Resolution};
use serde::Deserialize;
use tokio::process::{Child, Command};
use tracing::{error, info, warn};
use ulid::Ulid;

#[derive(Parser)]
#[command(name = "ersha-dispatchers-harness")]
#[command(about = "Spawn multiple ersha-dispatch processes across Ethiopia")]
struct Cli {
    /// Path to the harness configuration file
    #[arg(short, long, default_value = "ersha-dispatchers-harness.toml")]
    config: PathBuf,
}

#[derive(Debug, Deserialize)]
struct HarnessConfig {
    dispatcher_count: usize,
    devices_per_dispatcher: usize,
    reading_interval_secs: u64,
    status_interval_secs: u64,
    upload_interval_secs: u64,
    base_http_port: u16,
    ersha_dispatch_bin: String,
    prime_rpc_addr: String,
    tls: TlsConfig,
}

#[derive(Debug, Deserialize)]
struct TlsConfig {
    cert: String,
    key: String,
    root_ca: String,
    domain: String,
}

impl HarnessConfig {
    fn load(path: &Path) -> color_eyre::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: HarnessConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

/// Absolute TLS paths for generated dispatcher configs.
struct AbsTlsConfig {
    cert: String,
    key: String,
    root_ca: String,
    domain: String,
}

/// Resolve the ersha-dispatch binary path.
fn resolve_dispatch_bin(bin: &str) -> color_eyre::Result<PathBuf> {
    let path = PathBuf::from(bin);
    if path.is_absolute() && path.exists() {
        return Ok(path);
    }
    // Try relative to cwd
    let cwd = std::env::current_dir()?;
    let relative = cwd.join(bin);
    if relative.exists() {
        return Ok(relative);
    }
    // Try looking in target/debug and target/release
    for profile in ["debug", "release"] {
        let candidate = cwd.join("target").join(profile).join("ersha-dispatch");
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    // Fall back to PATH lookup
    Ok(PathBuf::from(bin))
}

/// Generate H3 resolution-10 cells spread across Ethiopia for dispatcher locations.
fn generate_dispatcher_locations(count: usize) -> Vec<u64> {
    let lat_min = 3.4_f64;
    let lat_max = 14.9_f64;
    let lng_min = 33.0_f64;
    let lng_max = 48.0_f64;

    let oversample = (count as f64 * 1.5).sqrt().ceil() as usize;
    let rows = oversample;
    let cols = oversample;

    let lat_step = (lat_max - lat_min) / rows as f64;
    let lng_step = (lng_max - lng_min) / cols as f64;

    let mut cells = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for r in 0..rows {
        for c in 0..cols {
            let lat = lat_min + (r as f64 + 0.5) * lat_step;
            let lng = lng_min + (c as f64 + 0.5) * lng_step;

            let ll = LatLng::new(lat, lng).expect("valid lat/lng for Ethiopia");
            let cell = ll.to_cell(Resolution::Ten);
            let cell_u64 = u64::from(cell);

            if seen.insert(cell_u64) {
                cells.push(cell_u64);
                if cells.len() == count {
                    return cells;
                }
            }
        }
    }

    cells
}

/// Write a per-dispatcher TOML config file and return its path.
fn write_dispatcher_config(
    dir: &Path,
    index: usize,
    dispatcher_id: &str,
    location: u64,
    http_port: u16,
    config: &HarnessConfig,
    tls: &AbsTlsConfig,
) -> color_eyre::Result<PathBuf> {
    let path = dir.join(format!("dispatcher-{index}.toml"));

    let content = format!(
        r#"[dispatcher]
id = "{dispatcher_id}"
location = {location:#018x}

[server]
http_addr = "0.0.0.0:{http_port}"

[storage]
type = "memory"

[prime]
rpc_addr = "{prime_rpc_addr}"
upload_interval_secs = {upload_interval_secs}

[edge]
type = "mock"
reading_interval_secs = {reading_interval_secs}
status_interval_secs = {status_interval_secs}
device_count = {device_count}

[tls]
cert = "{cert}"
key = "{key}"
root_ca = "{root_ca}"
domain = "{domain}"
"#,
        prime_rpc_addr = config.prime_rpc_addr,
        upload_interval_secs = config.upload_interval_secs,
        reading_interval_secs = config.reading_interval_secs,
        status_interval_secs = config.status_interval_secs,
        device_count = config.devices_per_dispatcher,
        cert = tls.cert,
        key = tls.key,
        root_ca = tls.root_ca,
        domain = tls.domain,
    );

    std::fs::write(&path, content)?;
    Ok(path)
}

/// Spawn an ersha-dispatch child process with the given config file.
async fn spawn_dispatcher(
    bin: &Path,
    config_path: &Path,
    index: usize,
) -> color_eyre::Result<Child> {
    let child = Command::new(bin)
        .arg("--config")
        .arg(config_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()?;

    info!(index, pid = child.id().unwrap_or(0), config = ?config_path, "Spawned dispatcher");
    Ok(child)
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "tracing=info,ersha_dispatchers_harness=info".to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .init();

    let cli = Cli::parse();

    let config = if cli.config.exists() {
        info!(path = ?cli.config, "Loading harness configuration");
        HarnessConfig::load(&cli.config)?
    } else {
        return Err(color_eyre::eyre::eyre!(
            "Configuration file not found: {:?}",
            cli.config
        ));
    };

    // Resolve the ersha-dispatch binary to an absolute path
    let dispatch_bin = resolve_dispatch_bin(&config.ersha_dispatch_bin)?;
    info!(bin = ?dispatch_bin, "Resolved ersha-dispatch binary");

    // Resolve TLS paths to absolute for generated configs
    let cwd = std::env::current_dir()?;
    let abs_tls = AbsTlsConfig {
        cert: cwd.join(&config.tls.cert).display().to_string(),
        key: cwd.join(&config.tls.key).display().to_string(),
        root_ca: cwd.join(&config.tls.root_ca).display().to_string(),
        domain: config.tls.domain.clone(),
    };

    info!(
        dispatcher_count = config.dispatcher_count,
        devices_per_dispatcher = config.devices_per_dispatcher,
        total_devices = config.dispatcher_count * config.devices_per_dispatcher,
        "Starting dispatchers harness"
    );

    // Generate dispatcher locations across Ethiopia
    let locations = generate_dispatcher_locations(config.dispatcher_count);
    info!(
        generated = locations.len(),
        requested = config.dispatcher_count,
        "Generated dispatcher locations"
    );

    // Create temp directory for config files
    let config_dir = std::env::temp_dir().join("ersha-harness-configs");
    if config_dir.exists() {
        std::fs::remove_dir_all(&config_dir)?;
    }
    std::fs::create_dir_all(&config_dir)?;
    info!(dir = ?config_dir, "Created config directory");

    // Generate configs and spawn processes
    let mut children: Vec<Child> = Vec::with_capacity(locations.len());
    let mut config_paths: Vec<PathBuf> = Vec::with_capacity(locations.len());

    for (i, &location) in locations.iter().enumerate() {
        let dispatcher_id = Ulid::new().to_string();
        let http_port = config.base_http_port + i as u16;

        let config_path = write_dispatcher_config(
            &config_dir,
            i,
            &dispatcher_id,
            location,
            http_port,
            &config,
            &abs_tls,
        )?;
        config_paths.push(config_path.clone());

        match spawn_dispatcher(&dispatch_bin, &config_path, i).await {
            Ok(child) => children.push(child),
            Err(e) => {
                error!(index = i, error = %e, "Failed to spawn dispatcher");
            }
        }
    }

    info!(
        spawned = children.len(),
        total = locations.len(),
        "All dispatchers launched"
    );

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("Received Ctrl+C, shutting down all dispatchers...");

    // Kill all children (kill_on_drop handles this, but we explicitly wait)
    for (i, child) in children.iter_mut().enumerate() {
        match child.kill().await {
            Ok(()) => info!(index = i, "Dispatcher stopped"),
            Err(e) => warn!(index = i, error = %e, "Failed to stop dispatcher"),
        }
    }

    // Clean up temp configs
    if let Err(e) = std::fs::remove_dir_all(&config_dir) {
        warn!(error = %e, "Failed to clean up config directory");
    } else {
        info!("Cleaned up config directory");
    }

    info!("Harness shutdown complete");
    Ok(())
}
