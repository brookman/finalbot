use clap::Parser;

#[derive(Parser)]
#[command(version, about)]
pub struct Args {
    /// X coordinate of the pixel to sample
    #[arg(default_value = "100")]
    x: u32,
    /// Y coordinate of the pixel to sample
    #[arg(default_value = "100")]
    y: u32,
}

impl Args {
    pub fn coordinates(&self) -> (u32, u32) {
        (self.x, self.y)
    }
}
