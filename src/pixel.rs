use image::RgbaImage;
use pipewire::spa::param::video::VideoFormat;

#[derive(Clone, Debug)]
pub struct BufferContext {
    pub offset: usize,
    pub stride: i32,
    pub width: usize,
    pub height: usize,
    pub format: VideoFormat,
}

impl BufferContext {
    pub fn to_rgba_image(&self, bytes: &[u8]) -> Option<RgbaImage> {
        let bpp = match self.format {
            VideoFormat::RGB => 3,
            VideoFormat::RGBA | VideoFormat::RGBx | VideoFormat::BGRx => 4,
            _ => return None,
        };

        let row_stride = if self.stride == 0 {
            self.width * bpp
        } else {
            self.stride.unsigned_abs() as usize
        };

        #[allow(clippy::cast_possible_truncation)]
        let mut img = RgbaImage::new(self.width as u32, self.height as u32);

        for y in 0..self.height {
            let src_y = if self.stride < 0 {
                self.height - 1 - y
            } else {
                y
            };
            let row_start = self.offset + src_y * row_stride;

            #[allow(clippy::cast_possible_truncation)]
            for x in 0..self.width {
                let src_idx = row_start + x * bpp;
                if src_idx + bpp > bytes.len() {
                    return None;
                }
                let pixel = match self.format {
                    VideoFormat::RGB | VideoFormat::RGBx => {
                        image::Rgba([bytes[src_idx], bytes[src_idx + 1], bytes[src_idx + 2], 255])
                    }
                    VideoFormat::RGBA => image::Rgba([
                        bytes[src_idx],
                        bytes[src_idx + 1],
                        bytes[src_idx + 2],
                        bytes[src_idx + 3],
                    ]),
                    VideoFormat::BGRx => {
                        image::Rgba([bytes[src_idx + 2], bytes[src_idx + 1], bytes[src_idx], 255])
                    }
                    _ => unreachable!(),
                };
                img.put_pixel(x as u32, y as u32, pixel);
            }
        }

        Some(img)
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
        let img = ctx.to_rgba_image(&buf).unwrap();
        assert_eq!(img.get_pixel(0, 0).0, [255u8, 0, 0, 255]);
        assert_eq!(img.get_pixel(1, 1).0, [128u8, 128, 128, 255]);
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
        let img = ctx.to_rgba_image(&buf).unwrap();
        assert_eq!(img.get_pixel(0, 0).0, [255u8, 0, 0, 255]);
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
        let img = ctx.to_rgba_image(&buf).unwrap();
        assert_eq!(img.get_pixel(1, 0).0, [0u8, 255, 0, 255]);
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
        assert!(ctx.to_rgba_image(&buf).is_some());
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
        assert!(ctx.to_rgba_image(&buf).is_none());
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
        let img = ctx.to_rgba_image(&buf).unwrap();
        assert_eq!(img.get_pixel(0, 0).0, [255u8, 0, 0, 255]);
        assert_eq!(img.get_pixel(0, 1).0, [0u8, 255, 0, 255]);
    }
}
