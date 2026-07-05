use crate::{SCREEN_H, SCREEN_W};
use anyhow::Result;
use evdevil::event::{Abs, AbsEvent, Key, KeyEvent, KeyState};
use evdevil::uinput::{AbsSetup, UinputDevice};
use evdevil::{AbsInfo, InputProp};
use std::time::Duration;

pub struct Mouse {
    device: UinputDevice,
}

const SLEEP_MS: u64 = 18;

impl Mouse {
    pub async fn new() -> Result<Mouse> {
        let device = UinputDevice::builder()?
            .with_props([InputProp::POINTER])?
            .with_abs_axes([
                AbsSetup::new(Abs::X, AbsInfo::new(0, SCREEN_W - 1)),
                AbsSetup::new(Abs::Y, AbsInfo::new(0, SCREEN_H - 1)),
            ])?
            .with_keys([Key::BTN_LEFT])?
            .build("rust-automation-pointer")?;

        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(Mouse { device })
    }

    pub fn move_to(&self, x: i32, y: i32) -> Result<()> {
        self.device.write_events(&[
            AbsEvent::new(Abs::X, x).into(),
            AbsEvent::new(Abs::Y, y).into(),
        ])?;
        Ok(())
    }

    pub async fn press_left(&self) -> Result<()> {
        self.device
            .write_events(&[KeyEvent::new(Key::BTN_LEFT, KeyState::PRESSED).into()])?;
        tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await;
        Ok(())
    }

    pub async fn release_left(&self) -> Result<()> {
        self.device
            .write_events(&[KeyEvent::new(Key::BTN_LEFT, KeyState::RELEASED).into()])?;
        tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await;
        Ok(())
    }

    pub async fn click_left(&self) -> Result<()> {
        self.press_left().await?;
        self.release_left().await?;
        Ok(())
    }

    pub async fn drag_left(&self, x_from: i32, y_from: i32, x_to: i32, y_to: i32) -> Result<()> {
        self.move_to(x_from, y_from)?;
        self.press_left().await?;
        self.move_to(x_to, y_to)?;
        self.release_left().await?;
        Ok(())
    }
}
