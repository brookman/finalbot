use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use evdev::{Device, EventType, KeyCode};

static CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn is_cancelled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}

fn first_event_device() -> Option<std::path::PathBuf> {
    let dir = std::fs::read_dir("/dev/input").ok()?;
    dir.filter_map(Result::ok).find_map(|e| {
        let p = e.path();
        p.to_string_lossy().contains("event").then_some(p)
    })
}

fn open_keyboard_devices() -> Vec<Device> {
    let Ok(dir) = std::fs::read_dir("/dev/input") else {
        return vec![];
    };

    dir.filter_map(|entry| {
        let path = entry.ok()?.path();
        let device = Device::open(&path).ok()?;
        device.set_nonblocking(true).ok()?;
        let keys = device.supported_keys()?;
        keys.contains(KeyCode::KEY_ESC).then_some(device)
    })
    .collect()
}

pub fn start() {
    thread::spawn(|| {
        let probe = first_event_device();
        let accessible = probe.is_some_and(|p| Device::open(&p).is_ok());

        if !accessible {
            eprintln!(
                "Warning: cannot read /dev/input/event* (Permission denied). \
                 Hotkey cancellation disabled.\n\
                 Hint: add yourself to the 'input' group or run as root:\n  \
                   sudo usermod -aG input $USER   # then log out and back in"
            );
            return;
        }

        let mut devices = open_keyboard_devices();

        if devices.is_empty() {
            eprintln!(
                "Warning: no keyboard devices found for hotkey listener. \
                 Auto-clicking cannot be cancelled with Escape."
            );
            return;
        }

        eprintln!(
            "[finalbot] Hotkey listener active — press Escape to cancel auto-clicking \
             ({} device(s))",
            devices.len()
        );

        loop {
            for device in &mut devices {
                let Ok(events) = device.fetch_events() else {
                    continue;
                };
                for event in events {
                    if event.event_type() == EventType::KEY
                        && event.code() == KeyCode::KEY_ESC.code()
                        && event.value() == 1
                    {
                        CANCELLED.store(true, Ordering::Relaxed);
                        eprintln!("[finalbot] Escape pressed — cancelling auto-clicking");
                    }
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
    });
}
