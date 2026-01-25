# ersha_edge

`ersha_edge` is a **no_std, async-first edge telemetry library** designed for
microcontrollers running Embassy. It provides a clean way to:

* Define sensors as async tasks
* Automatically register sensors with a backend
* Encode and send framed sensor readings
* Handle reconnects and transient network failures
* Keep your application logic simple and declarative

It is designed for **real devices** (RP2040 / RP235x class MCUs) and **real
networks** (WiFi, later LoRaWAN, BLE, etc.).

---

## Mental model

Think of `ersha_edge` as three layers:

1. **Sensors** – small, async producers of metrics
2. **Engine** – collects, frames, and sends readings
3. **Transport** – how bytes actually leave the device

You write *sensors*.  You choose a *transport*.  The *engine* does the rest.

---

## Quick example (RP235x + WiFi)

Below is a **complete working example** showing how `ersha_edge` is used in an
edge client running on an RP235x-class board using WiFi.

This example demonstrates:

* defining sensors
* spawning sensor tasks
* bringing up WiFi
* running the engine

```rust
#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_time::Duration;
use ersha_edge::{
    Engine, Sensor, SensorMetric,
    sensor::{SensorConfig, SensorError},
    sensor_task,
    transport::Wifi,
};

// ---------------- Sensors ----------------

pub struct MockSoilMoistureSensor;

impl Sensor for MockSoilMoistureSensor {
    fn config(&self) -> SensorConfig {
        SensorConfig {
            sampling_rate: Duration::from_secs(1),
        }
    }

    async fn read(&self) -> Result<SensorMetric, SensorError> {
        Ok(SensorMetric::SoilMoisture(32))
    }
}

pub struct MockTempSensor;

impl Sensor for MockTempSensor {
    fn config(&self) -> SensorConfig {
        SensorConfig {
            sampling_rate: Duration::from_secs(5),
        }
    }

    async fn read(&self) -> Result<SensorMetric, SensorError> {
        Ok(SensorMetric::AirTemp(12))
    }
}

// Generate Embassy tasks for sensors
sensor_task!(soil_moisture, MockSoilMoistureSensor);
sensor_task!(air_temperature, MockTempSensor);

// ---------------- Main ----------------

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // (WiFi + network stack setup omitted here for brevity)
    // See full example in `examples/rp235x_wifi.rs`

    let wifi = Wifi::new(stack, rx_buffer, tx_buffer);
    let engine = Engine::new(wifi).await.unwrap();

    spawner.spawn(unwrap!(soil_moisture(&MockSoilMoistureSensor)));
    spawner.spawn(unwrap!(air_temperature(&MockTempSensor)));
    spawner.spawn(unwrap!(ersha_wifi(engine)));
}

#[embassy_executor::task]
async fn ersha_wifi(engine: Engine<Wifi<'static>>) {
    engine.run().await
}
```

---

## Defining a sensor

A sensor is anything that implements the `Sensor` trait:

```rust
pub trait Sensor {
    fn config(&self) -> SensorConfig;
    async fn read(&self) -> Result<SensorMetric, SensorError>;
}
```

### Key points

* `config()` defines **how often** the sensor is sampled
* `read()` is async and can talk to hardware
* You return a strongly-typed `SensorMetric`

No IDs. No networking. No serialization.

---

## Sensor metrics

Metrics are represented as an enum:

```rust
pub enum SensorMetric {
    SoilMoisture(u8),
    AirTemp(i16),
    Humidity(u8),
}
```

This gives you:

* compile-time safety
* predictable encoding
* easy decoding on the server side

---

## Sensor tasks

Sensors are turned into Embassy tasks using a macro:

```rust
sensor_task!(soil_moisture, MockSoilMoistureSensor);
```

This macro:

* registers the sensor with the engine
* assigns a unique sensor ID
* periodically reads the sensor
* sends readings to the engine

You never manage IDs manually.

---

## The Engine

The `Engine` is responsible for:

* provisioning the device
* framing packets
* handling disconnects and reconnects
* retrying sends

You create it once:

```rust
let engine = Engine::new(transport).await?;
```

And then run it forever:

```rust
engine.run().await
```

The engine is **transport-agnostic**.

---

## Transports

A transport defines *how bytes leave the device*.

Currently supported:

* `Wifi` (TCP-based)

Planned:

* LoRaWAN
* BLE

Example:

```rust
let wifi = Wifi::new(stack, rx_buffer, tx_buffer);
let engine = Engine::new(wifi).await?;
```

Transports automatically handle:

* reconnects
* framing boundaries
* backpressure

---

## Handling disconnects

`ersha_edge` is designed for unstable networks:

* sensor tasks keep running
* engine buffers and retries
* reconnects are transparent

Your application code does **not** need special handling.

---

## What you *don’t* have to do

With `ersha_edge`, you do **not**:

* assign sensor IDs
* manually serialize packets
* manage sockets per sensor
* restart tasks on disconnect

You focus on **data**, not plumbing.
