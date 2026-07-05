use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use evdev::{Device, EventType, InputEvent, KeyCode};
use futures::StreamExt;
use tokio::sync::Notify;
use tracing::{info, warn};

pub struct Cancel {
    cancelled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl Cancel {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub async fn wait(&self) {
        let notified = self.notify.notified();
        if !self.is_cancelled() {
            notified.await;
        }
    }
}

fn is_escape_press(event: &InputEvent) -> bool {
    event.event_type() == EventType::KEY
        && event.code() == KeyCode::KEY_ESC.code()
        && event.value() == 1
}

pub fn start() -> Cancel {
    let cancelled = Arc::new(AtomicBool::new(false));
    let notify = Arc::new(Notify::new());

    let Some(devices) = find_keyboards() else {
        warn!("/dev/input not found. Hotkey disabled.");
        return Cancel { cancelled, notify };
    };

    if devices.is_empty() {
        warn!("no keyboard devices found. Hotkey disabled.");
        return Cancel { cancelled, notify };
    }

    let streams: Vec<_> = devices
        .into_iter()
        .filter_map(|d| d.into_event_stream().ok())
        .collect();

    if streams.is_empty() {
        warn!("could not open input event streams. Hotkey disabled.");
        return Cancel { cancelled, notify };
    }

    info!(
        "Hotkey active — press Escape to cancel ({} device(s))",
        streams.len()
    );

    let c_cancelled = Arc::clone(&cancelled);
    let c_notify = Arc::clone(&notify);
    tokio::spawn(async move {
        let mut merged = futures::stream::select_all(streams);
        loop {
            match merged.next().await {
                Some(Ok(event)) if is_escape_press(&event) => {
                    info!("Escape pressed — cancelling");
                    c_cancelled.store(true, Ordering::Relaxed);
                    c_notify.notify_one();
                    break;
                }
                Some(Err(e)) => {
                    warn!("input event error: {e}");
                }
                None => break,
                _ => {}
            }
        }
    });

    Cancel { cancelled, notify }
}

fn find_keyboards() -> Option<Vec<Device>> {
    let dir = std::fs::read_dir("/dev/input").ok()?;
    Some(
        dir.filter_map(|entry| {
            let path = entry.ok()?.path();
            let device = Device::open(&path).ok()?;
            let keys = device.supported_keys()?;
            keys.contains(KeyCode::KEY_ESC).then_some(device)
        })
        .collect(),
    )
}
