//! `PipeWire` screen capture pixel sampler.
//!
//! Opens a screencast portal via [`portal::open_portal`], connects a `PipeWire`
//! stream via [`pipewire::start`], and prints the RGBA value of a single
//! pixel from every received frame. The pixel coordinates are read from
//! `argv[1]` and `argv[2]` (defaults 100 each).

// `u32 as usize` is lossless on all real targets but clippy's
// `cast_lossless` can't express it (no `From<u32>` impl for `usize`).
#![allow(clippy::cast_lossless)]

mod pipewire;
mod pixel;
mod portal;

use anyhow::Result;
use pixel::BufferContext;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::Level::INFO.into())
                .from_env_lossy(),
        )
        .init();
    let sample_x = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    let sample_y = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let (stream, fd) = portal::open_portal().await?;
    let node_id = stream.pipe_wire_node_id();

    tracing::info!("PipeWire node id: {node_id}");
    pipewire::start(node_id, fd, move |ctx: &BufferContext, bytes: &[u8]| {
        if let Some(pixel) = ctx.sample_pixel(bytes, sample_x, sample_y) {
            println!(
                "rgba({}, {}, {}, {})",
                pixel[0], pixel[1], pixel[2], pixel[3]
            );
        } else {
            tracing::warn!("could not sample pixel at ({sample_x}, {sample_y})");
        }
    })?;
    Ok(())
}
