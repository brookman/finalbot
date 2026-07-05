//! `PipeWire` screen capture pixel sampler.
//!
//! Opens a screencast portal via [`portal::open_portal`], connects a `PipeWire`
//! stream via [`pipewire::start`], and prints the RGBA value of a single
//! pixel from every received frame.

// `u32 as usize` is lossless on all real targets but clippy's
// `cast_lossless` can't express it (no `From<u32>` impl for `usize`).
#![allow(clippy::cast_lossless)]

mod args;
mod hotkey;
mod mouse;
mod pipewire;
mod pixel;
mod portal;

use crate::args::Args;
use anyhow::{Context, Result};
use clap::Parser;
use pixel::BufferContext;
use tracing::{info, warn, Level};

use std::time::Duration;

use crate::mouse::Mouse;

const SCREEN_W: i32 = 2560;
const SCREEN_H: i32 = 1440;

#[tokio::main]
async fn main() -> Result<()> {
    let (x, y) = init(Level::INFO)?.coordinates();

    let mut cancel = hotkey::start();

    let mouse = Mouse::new().await?;

    mouse.move_to(1700, 900)?;
    let _ = mouse.click_left().await;
    tokio::time::sleep(Duration::from_millis(180)).await;

    if !cancel.is_cancelled() {
        let _ = mouse.click_left().await;
        tokio::time::sleep(Duration::from_millis(180)).await;
    }

    for _ in 0..100 {
        if cancel.is_cancelled() {
            info!("Auto-clicking cancelled");
            break;
        }

        tokio::select! {
            () = cancel.wait() => {
                info!("Auto-clicking cancelled");
                break;
            }
            result = async {
                mouse.drag_left(1900, 550, 1900, 450).await?;
                mouse.move_to(1700, 1000)?;
                let _ = mouse.click_left().await;
                Ok::<_, anyhow::Error>(())
            } => {
                result?;
            }
        }
    }

    let (stream, fd) = portal::open_portal().await?;
    let node_id = stream.pipe_wire_node_id();
    info!("PipeWire node id: {node_id}");

    pipewire::start(node_id, fd, move |ctx: &BufferContext, bytes: &[u8]| {
        if let Some(pixel) = ctx.sample_pixel(bytes, x, y) {
            info!(
                "rgba({}, {}, {}, {})",
                pixel[0], pixel[1], pixel[2], pixel[3]
            );
        } else {
            warn!("could not sample pixel at ({x}, {y})");
        }
    })?;

    Ok(())
}

fn init(log_level: Level) -> Result<Args> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(log_level.into())
                .from_env_lossy(),
        )
        .init();

    Args::try_parse().context("Could not parse args")
}
