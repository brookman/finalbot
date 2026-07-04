mod pipewire;
mod pixel;
mod portal;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
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

    eprintln!("PipeWire node id: {node_id}");
    pipewire::start(node_id, fd, sample_x, sample_y)?;
    Ok(())
}
