use evdev::{Device, EventType, InputEvent, KeyCode};
use futures::StreamExt;
use tokio::sync::watch;
use tracing::info;

pub struct Cancel {
    rx: watch::Receiver<bool>,
    _tx: watch::Sender<bool>,
}

impl Cancel {
    pub fn is_cancelled(&self) -> bool {
        *self.rx.borrow()
    }

    pub async fn wait(&mut self) {
        if self.is_cancelled() {
            return;
        }
        let _ = self.rx.changed().await;
    }
}

fn is_escape_press(event: &InputEvent) -> bool {
    event.event_type() == EventType::KEY
        && event.code() == KeyCode::KEY_ESC.code()
        && event.value() == 1
}

pub fn start() -> Cancel {
    let (tx, rx) = watch::channel(false);

    let Some(devices) = find_keyboards() else {
        eprintln!("Warning: /dev/input not found. Hotkey disabled.");
        return Cancel { rx, _tx: tx };
    };

    if devices.is_empty() {
        eprintln!("Warning: no keyboard devices found. Hotkey disabled.");
        return Cancel { rx, _tx: tx };
    }

    let streams: Vec<_> = devices
        .into_iter()
        .filter_map(|d| d.into_event_stream().ok())
        .collect();

    if streams.is_empty() {
        eprintln!("Warning: could not open input event streams. Hotkey disabled.");
        return Cancel { rx, _tx: tx };
    }

    info!(
        "Hotkey active — press Escape to cancel ({} device(s))",
        streams.len()
    );

    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let mut merged = futures::stream::select_all(streams);
        loop {
            match merged.next().await {
                Some(Ok(event)) if is_escape_press(&event) => {
                    info!("Escape pressed — cancelling");
                    let _ = tx_clone.send(true);
                    break;
                }
                Some(Err(e)) => {
                    tracing::warn!("input event error: {e}");
                }
                None => break,
                _ => {}
            }
        }
    });

    Cancel { rx, _tx: tx }
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
