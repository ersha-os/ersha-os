#![no_std]

mod sensor_registry;
use core::mem::MaybeUninit;

pub use sensor_registry::register_sensor;

use defmt::{Format, error};
use embassy_net::tcp::State;
use embassy_net::{IpAddress, IpEndpoint, Stack, tcp::TcpSocket};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender};
use embassy_time::{Duration, Timer};
use serde::{Deserialize, Serialize};

const READING_QUEUE_DEPTH: usize = 16;

const SERVER_ADDR: IpEndpoint = IpEndpoint {
    addr: IpAddress::v4(10, 46, 238, 14),
    port: 9001,
};

// TODO: consolidate with ersha-core::SensorMetric after
// resolveing no_std issues.
#[derive(Serialize, Deserialize, Debug, Clone, Format)]
pub enum SensorMetric {
    /// Percentage 0-100 (1 byte in Postcard)
    SoilMoisture(u8),
    /// Degrees Celsius scaled by 100 (e.g., 25.43 -> 2543).
    /// Fits in 2 bytes instead of 4.
    SoilTemp(i16),
    AirTemp(i16),
    Humidity(u8),
    /// Rainfall in mm scaled by 100.
    Rainfall(u16),
}

pub type DeviceId = u32;
pub type SensorId = u8;
pub type ReadingId = u16;

#[derive(Serialize, Deserialize, Format, Clone, Copy)]
pub struct SensorCapability {
    pub sensor_id: SensorId,
    pub metric: SensorMetricKind,
}

#[derive(Serialize, Deserialize, Format, Clone, Copy)]
pub enum SensorMetricKind {
    SoilMoisture,
    SoilTemp,
    AirTemp,
    Humidity,
    Rainfall,
}

#[derive(Serialize, Deserialize, Format)]
pub struct ReadingPacket {
    pub device_id: DeviceId,
    pub sensor_id: SensorId,
    pub reading_id: ReadingId,
    pub metric: SensorMetric,
}

#[derive(Clone, Format)]
pub struct TaggedReading {
    pub sensor_id: SensorId,
    pub metric: SensorMetric,
}

static READING_CHANNEL: Channel<CriticalSectionRawMutex, TaggedReading, READING_QUEUE_DEPTH> =
    Channel::new();

pub fn sender() -> Sender<'static, CriticalSectionRawMutex, TaggedReading, READING_QUEUE_DEPTH> {
    READING_CHANNEL.sender()
}

pub struct SensorConfig {
    pub sampling_rate: Duration,
    pub calibration_offset: f32,
}

#[derive(defmt::Format)]
pub enum SensorError {
    Timeout,
    InvalidData,
}

pub trait Sensor {
    fn config(&self) -> SensorConfig;
    fn read(&self) -> impl Future<Output = Result<SensorMetric, SensorError>>;
}

#[derive(Serialize, Deserialize, Debug, Format)]
pub struct UplinkPacket {
    pub seq: u8,
    pub sensor_id: u8,
    pub metric: SensorMetric,
}

#[derive(Debug, Format)]
pub enum Error {
    UnableToSend,
    SerializationFailed,
    ServerNotFound,
    TooManySensors,
}

pub trait Transport {
    /// Called once after network join / connect
    fn provision(&mut self) -> impl Future<Output = Result<DeviceId, Error>>;

    /// Send device capabilities to the server / network
    fn announce_sensors(
        &mut self,
        device_id: DeviceId,
        sensors: &[SensorCapability],
    ) -> impl Future<Output = Result<(), Error>>;

    /// Send a single sensor reading
    fn send_reading(&mut self, packet: &ReadingPacket) -> impl Future<Output = Result<(), Error>>;
}

pub struct Engine<T: Transport> {
    transport: T,
    device_id: DeviceId,
    reading_seq: ReadingId,
}

impl<T: Transport> Engine<T> {
    pub async fn new(mut transport: T) -> Result<Self, Error> {
        let device_id = transport.provision().await?;

        let registry = sensor_registry::SENSOR_REGISTRY.lock().await;

        let mut caps_buf: [MaybeUninit<SensorCapability>; sensor_registry::MAX_SENSORS] =
            unsafe { MaybeUninit::uninit().assume_init() };

        let mut count = 0;

        for cap in registry.capabilities() {
            caps_buf[count].write(cap);
            count += 1;
        }

        let caps: &[SensorCapability] = unsafe {
            core::slice::from_raw_parts(caps_buf.as_ptr() as *const SensorCapability, count)
        };

        transport.announce_sensors(device_id, caps).await?;

        Ok(Self {
            transport,
            device_id,
            reading_seq: 0,
        })
    }

    pub async fn run(mut self) -> ! {
        let receiver = READING_CHANNEL.receiver();

        loop {
            let reading = receiver.receive().await;

            let packet = ReadingPacket {
                device_id: self.device_id,
                sensor_id: reading.sensor_id,
                reading_id: self.reading_seq,
                metric: reading.metric,
            };

            if let Err(e) = self.transport.send_reading(&packet).await {
                error!("Uplink failed: {:?}", e);
            }

            self.reading_seq = self.reading_seq.wrapping_add(1);
            Timer::after_millis(100).await;
        }
    }
}

#[macro_export]
macro_rules! sensor_task {
    ($task_name:ident, $sensor_ty:ty, $metric_kind:expr) => {
        #[embassy_executor::task]
        async fn $task_name(sensor: &'static $sensor_ty) -> ! {
            let sender = $crate::sender();

            let sensor_id = match $crate::register_sensor($metric_kind).await {
                Ok(id) => id,
                Err(e) => {
                    defmt::error!("Sensor registration failed: {:?}", e);
                    loop {
                        embassy_time::Timer::after_secs(10).await;
                    }
                }
            };

            loop {
                let config = sensor.config();

                match sensor.read().await {
                    Ok(reading) => {
                        let reading = $crate::TaggedReading {
                            sensor_id,
                            metric: reading,
                        };

                        if sender.try_send(reading).is_err() {
                            defmt::warn!("Sensor queue full, dropping reading");
                        };
                    }
                    Err(e) => {
                        error!("Sender Error: {:?}", e);
                    }
                }

                Timer::after(config.sampling_rate).await;
            }
        }
    };
}

pub struct Wifi<'a> {
    socket: TcpSocket<'a>,
    device_id: Option<DeviceId>,
}

impl<'a> Wifi<'a> {
    pub fn new(stack: Stack<'a>, rx: &'a mut [u8], tx: &'a mut [u8]) -> Self {
        Self {
            socket: TcpSocket::new(stack, rx, tx),
            device_id: None,
        }
    }
}

async fn read_exact(socket: &mut TcpSocket<'_>, mut buf: &mut [u8]) -> Result<(), Error> {
    while !buf.is_empty() {
        let n = socket.read(buf).await.map_err(|_| Error::UnableToSend)?;

        if n == 0 {
            return Err(Error::ServerNotFound);
        }

        buf = &mut buf[n..];
    }
    Ok(())
}

impl<'a> Transport for Wifi<'a> {
    async fn provision(&mut self) -> Result<DeviceId, Error> {
        if self.socket.state() != State::Established {
            self.socket
                .connect(SERVER_ADDR)
                .await
                .map_err(|_| Error::ServerNotFound)?;
        }

        self.socket
            .write(b"HELLO")
            .await
            .map_err(|_| Error::UnableToSend)?;

        let mut buf = [0u8; 4];
        read_exact(&mut self.socket, &mut buf)
            .await
            .map_err(|_| Error::UnableToSend)?;

        let id = u32::from_be_bytes(buf);
        self.device_id = Some(id);
        Ok(id)
    }

    async fn announce_sensors(
        &mut self,
        device_id: DeviceId,
        sensors: &[SensorCapability],
    ) -> Result<(), Error> {
        let mut buf = [0u8; 64];
        let used = postcard::to_slice(&(device_id, sensors), &mut buf)
            .map_err(|_| Error::SerializationFailed)?;

        self.socket
            .write(used)
            .await
            .map_err(|_| Error::UnableToSend)?;
        Ok(())
    }

    async fn send_reading(&mut self, packet: &ReadingPacket) -> Result<(), Error> {
        let mut buf = [0u8; 64];
        let used = postcard::to_slice(packet, &mut buf).map_err(|_| Error::SerializationFailed)?;

        self.socket
            .write(used)
            .await
            .map_err(|_| Error::UnableToSend)?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use embassy_time::Duration;

    struct MockSoilSensor;

    impl Sensor for MockSoilSensor {
        fn config(&self) -> SensorConfig {
            SensorConfig {
                sampling_rate: Duration::from_millis(10),
                calibration_offset: 0.0,
            }
        }

        fn read(&self) -> impl core::future::Future<Output = Result<SensorMetric, SensorError>> {
            async move { Ok(SensorMetric::SoilMoisture(42)) }
        }
    }

    sensor_task!(soil_task, MockSoilSensor, SensorMetricKind::SoilMoisture);
}
