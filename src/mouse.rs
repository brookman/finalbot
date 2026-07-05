use crate::{SCREEN_H, SCREEN_W};
use anyhow::Result;
use evdevil::event::{Abs, AbsEvent, Key, KeyEvent, KeyState};
use evdevil::uinput::{AbsSetup, UinputDevice};
use evdevil::{AbsInfo, InputProp};
use std::thread;
use std::time::Duration;

pub struct Mouse {
    device: UinputDevice,
}

const SLEEP: u64 = 18;

impl Mouse {
    pub fn new() -> Result<Mouse> {
        let device = UinputDevice::builder()?
            .with_props([InputProp::POINTER])?
            .with_abs_axes([
                AbsSetup::new(Abs::X, AbsInfo::new(0, SCREEN_W - 1)),
                AbsSetup::new(Abs::Y, AbsInfo::new(0, SCREEN_H - 1)),
            ])?
            .with_keys([Key::BTN_LEFT])?
            .build("rust-automation-pointer")?;

        // Give the compositor/libinput a moment to notice the new device.
        thread::sleep(Duration::from_millis(100));

        Ok(Mouse { device })
    }

    pub fn move_to(&self, x: i32, y: i32) -> Result<()> {
        self.device.write_events(&[
            AbsEvent::new(Abs::X, x).into(),
            AbsEvent::new(Abs::Y, y).into(),
        ])?;
        //thread::sleep(Duration::from_millis(SLEEP));
        Ok(())
    }

    pub fn press_left(&self) -> Result<()> {
        self.device
            .write_events(&[KeyEvent::new(Key::BTN_LEFT, KeyState::PRESSED).into()])?;
        thread::sleep(Duration::from_millis(SLEEP));
        Ok(())
    }

    pub fn release_left(&self) -> Result<()> {
        self.device
            .write_events(&[KeyEvent::new(Key::BTN_LEFT, KeyState::RELEASED).into()])?;
        thread::sleep(Duration::from_millis(SLEEP));
        Ok(())
    }

    pub fn click_left(&self) -> Result<()> {
        self.press_left()?;
        self.release_left()?;
        Ok(())
    }

    pub fn drag_left(&self, x_from: i32, y_from: i32, x_to: i32, y_to: i32) -> Result<()> {
        self.move_to(x_from, y_from)?;
        self.press_left()?;
        self.move_to(x_to, y_to)?;
        self.release_left()?;
        Ok(())
    }
}
