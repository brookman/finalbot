//! `PipeWire` screen capture pixel sampler.
//!
//! Opens a screencast portal via [`portal::open_portal`], connects a `PipeWire`
//! stream via [`pipewire::start`], and prints the RGBA value of a single
//! pixel from every received frame.

// `u32 as usize` is lossless on all real targets but clippy's
// `cast_lossless` can't express it (no `From<u32>` impl for `usize`).
#![allow(clippy::cast_lossless)]

mod args;
mod pipewire;
mod pixel;
mod portal;

use crate::args::Args;
use anyhow::{Context, Result};
use clap::Parser;
use pixel::BufferContext;
use tracing::{Level, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    let (x, y) = init(Level::INFO)?.coordinates();

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
