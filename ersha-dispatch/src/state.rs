use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use ersha_core::{AlertRequest, DeviceId, DisconnectionReason};

/// Events to be sent to ersha-prime.
#[derive(Debug, Clone)]
pub enum PrimeEvent {
    DeviceDisconnection {
        device_id: DeviceId,
        reason: DisconnectionReason,
    },
    Alert(AlertRequest),
}

/// Shared state for tracking devices and pending events.
pub struct DispatcherState {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    connected_devices: HashSet<DeviceId>,
    pending_events: Vec<PrimeEvent>,
    startup_time: Instant,
}

impl DispatcherState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                connected_devices: HashSet::new(),
                pending_events: Vec::new(),
                startup_time: Instant::now(),
            })),
        }
    }

    /// Record that a device has connected.
    pub async fn device_connected(&self, device_id: DeviceId) {
        let mut inner = self.inner.lock().await;
        inner.connected_devices.insert(device_id);
    }

    /// Record that a device has disconnected and queue a disconnection event.
    pub async fn device_disconnected(&self, device_id: DeviceId, reason: DisconnectionReason) {
        let mut inner = self.inner.lock().await;
        inner.connected_devices.remove(&device_id);
        inner
            .pending_events
            .push(PrimeEvent::DeviceDisconnection { device_id, reason });
    }

    /// Queue an alert to be sent to prime.
    pub async fn queue_alert(&self, alert: AlertRequest) {
        let mut inner = self.inner.lock().await;
        inner.pending_events.push(PrimeEvent::Alert(alert));
    }

    /// Take all pending events, leaving the queue empty.
    pub async fn take_pending_events(&self) -> Vec<PrimeEvent> {
        let mut inner = self.inner.lock().await;
        std::mem::take(&mut inner.pending_events)
    }

    /// Get the number of currently connected devices.
    pub async fn connected_count(&self) -> u32 {
        let inner = self.inner.lock().await;
        inner.connected_devices.len() as u32
    }

    /// Get the dispatcher uptime in seconds.
    pub async fn uptime_secs(&self) -> u64 {
        let inner = self.inner.lock().await;
        inner.startup_time.elapsed().as_secs()
    }
}

impl Default for DispatcherState {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DispatcherState {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
