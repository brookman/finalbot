//! Conversion of `PipeWire` raw video buffers into image crate types.
//!
//! [`BufferContext`] describes the layout of a raw video frame.
//! Use [`sample_pixel`](BufferContext::sample_pixel) to read a single pixel
//! without copying, or [`to_rgba_image`](BufferContext::to_rgba_image) to
//! produce a full [`RgbaImage`].

use pipewire::spa::param::video::VideoFormat;

/// Layout of a raw `PipeWire` video buffer.
#[derive(Clone, Debug)]
pub struct BufferContext {
    pub offset: usize,
    pub stride: i32,
    pub width: u32,
    pub height: u32,
    pub format: VideoFormat,
}

impl BufferContext {
    /// Number of bytes per pixel for the current format.
    fn bytes_per_pixel(&self) -> Option<usize> {
        match self.format {
            VideoFormat::RGB => Some(3),
            VideoFormat::RGBA | VideoFormat::RGBx | VideoFormat::BGRx => Some(4),
            _ => None,
        }
    }

    /// Byte stride between consecutive rows.
    fn row_stride(&self, bpp: usize) -> usize {
        if self.stride == 0 {
            self.width as usize * bpp
        } else {
            self.stride.unsigned_abs() as usize
        }
    }

    /// Source buffer row index for logical row `y`.
    fn src_row(&self, y: u32) -> usize {
        if self.stride < 0 {
            self.height.saturating_sub(1).saturating_sub(y) as usize
        } else {
            y as usize
        }
    }

    /// Read a single pixel without copying the entire buffer.
    pub fn sample_pixel(&self, bytes: &[u8], x: u32, y: u32) -> Option<image::Rgba<u8>> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let bpp = self.bytes_per_pixel()?;
        let idx = self.offset + self.src_row(y) * self.row_stride(bpp) + x as usize * bpp;
        let px = bytes.get(idx..idx + bpp)?;

        match self.format {
            VideoFormat::RGB | VideoFormat::RGBx => Some(image::Rgba([px[0], px[1], px[2], 255])),
            VideoFormat::RGBA => Some(image::Rgba([px[0], px[1], px[2], px[3]])),
            VideoFormat::BGRx => Some(image::Rgba([px[2], px[1], px[0], 255])),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rgba() {
        let ctx = BufferContext {
            offset: 0,
            stride: 8,
            width: 2,
            height: 2,
            format: VideoFormat::RGBA,
        };
        let buf = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 128, 128, 128, 255,
        ];
        assert_eq!(
            ctx.sample_pixel(&buf, 0, 0),
            Some(image::Rgba([255, 0, 0, 255]))
        );
        assert_eq!(
            ctx.sample_pixel(&buf, 1, 1),
            Some(image::Rgba([128, 128, 128, 255]))
        );
    }

    #[test]
    fn sample_bgrx() {
        let ctx = BufferContext {
            offset: 0,
            stride: 8,
            width: 2,
            height: 1,
            format: VideoFormat::BGRx,
        };
        let buf = vec![0, 0, 255, 255, 255, 0, 0, 255];
        assert_eq!(
            ctx.sample_pixel(&buf, 0, 0),
            Some(image::Rgba([255, 0, 0, 255]))
        );
    }

    #[test]
    fn sample_rgb() {
        let ctx = BufferContext {
            offset: 0,
            stride: 6,
            width: 2,
            height: 1,
            format: VideoFormat::RGB,
        };
        let buf = vec![255, 0, 0, 0, 255, 0];
        assert_eq!(
            ctx.sample_pixel(&buf, 1, 0),
            Some(image::Rgba([0, 255, 0, 255]))
        );
    }

    #[test]
    fn out_of_bounds() {
        let ctx = BufferContext {
            offset: 0,
            stride: 4,
            width: 1,
            height: 1,
            format: VideoFormat::RGBA,
        };
        let buf = vec![255; 4];
        assert!(ctx.sample_pixel(&buf, 0, 1).is_none());
    }

    #[test]
    fn unsupported_format() {
        let ctx = BufferContext {
            offset: 0,
            stride: 4,
            width: 1,
            height: 1,
            format: VideoFormat::I420,
        };
        let buf = vec![255; 4];
        assert!(ctx.sample_pixel(&buf, 0, 0).is_none());
    }

    #[test]
    fn negative_stride() {
        let ctx = BufferContext {
            offset: 0,
            stride: -4,
            width: 1,
            height: 2,
            format: VideoFormat::RGBA,
        };
        let buf = vec![0, 255, 0, 255, 255, 0, 0, 255];
        assert_eq!(
            ctx.sample_pixel(&buf, 0, 0),
            Some(image::Rgba([255, 0, 0, 255]))
        );
        assert_eq!(
            ctx.sample_pixel(&buf, 0, 1),
            Some(image::Rgba([0, 255, 0, 255]))
        );
    }
}
