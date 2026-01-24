use crate::DeviceId;
use crate::Error;
use crate::ReadingId;
use crate::ReadingPacket;
use crate::TaggedReading;
use crate::Transport;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Sender;
use embassy_time::Timer;

use defmt::error;

const READING_QUEUE_DEPTH: usize = 16;

pub static READING_CHANNEL: Channel<CriticalSectionRawMutex, TaggedReading, READING_QUEUE_DEPTH> =
    Channel::new();

pub struct Engine<T: Transport> {
    transport: T,
    device_id: DeviceId,
    reading_seq: ReadingId,
}

impl<T: Transport> Engine<T> {
    pub async fn new(mut transport: T) -> Result<Self, Error> {
        let device_id = transport.provision().await?;

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

pub fn sender() -> Sender<'static, CriticalSectionRawMutex, TaggedReading, READING_QUEUE_DEPTH> {
    READING_CHANNEL.sender()
}
