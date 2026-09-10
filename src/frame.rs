use anyhow::{Context, Result, ensure};

pub struct Frame {
    pub width: u16,
    pub height: u16,
    pub rgb: Vec<u8>,
}
impl Frame {
    pub fn decode(jpeg: &[u8]) -> Result<Self> {
        let mut decoder = jpeg_decoder::Decoder::new(jpeg);
        decoder.read_info()?;
        let info = decoder.info().context("JPEG has no dimensions")?;
        ensure!(
            info.width > 0
                && info.height > 0
                && u32::from(info.width) * u32::from(info.height) <= 24_000_000,
            "Unsupported JPEG dimensions"
        );
        ensure!(
            info.pixel_format == jpeg_decoder::PixelFormat::RGB24,
            "Expected RGB24 JPEG"
        );
        let rgb = decoder.decode()?;
        ensure!(
            rgb.len() == info.width as usize * info.height as usize * 3,
            "JPEG pixel length mismatch"
        );
        Ok(Self {
            width: info.width,
            height: info.height,
            rgb,
        })
    }
    pub fn rotate(self, degrees: u16) -> Result<Self> {
        ensure!(
            [0, 90, 180, 270].contains(&degrees),
            "Rotation must be 0, 90, 180 or 270 degrees clockwise"
        );
        if degrees == 0 {
            return Ok(self);
        }
        let (w, h) = (self.width as usize, self.height as usize);
        ensure!(self.rgb.len() == w * h * 3, "Invalid frame storage");
        let (out_w, out_h) = if degrees == 180 { (w, h) } else { (h, w) };
        let mut rgb = vec![0; self.rgb.len()];
        for y in 0..h {
            for x in 0..w {
                let (dx, dy) = match degrees {
                    90 => (h - 1 - y, x),
                    180 => (w - 1 - x, h - 1 - y),
                    _ => (y, w - 1 - x),
                };
                rgb[(dy * out_w + dx) * 3..(dy * out_w + dx + 1) * 3]
                    .copy_from_slice(&self.rgb[(y * w + x) * 3..(y * w + x + 1) * 3]);
            }
        }
        Ok(Self {
            width: out_w as u16,
            height: out_h as u16,
            rgb,
        })
    }
    pub fn jpeg(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        jpeg_encoder::Encoder::new(&mut data, 95).encode(
            &self.rgb,
            self.width,
            self.height,
            jpeg_encoder::ColorType::Rgb,
        )?;
        Ok(data)
    }
    pub fn crop(self, aspect: Aspect) -> Result<Self> {
        let (w, h) = (u32::from(self.width), u32::from(self.height));
        let (mut cw, mut ch) = match aspect {
            Aspect::Native => (w, h),
            Aspect::Landscape if w * 9 > h * 16 => (h * 16 / 9, h),
            Aspect::Landscape => (w, w * 9 / 16),
            Aspect::Portrait if w * 16 > h * 9 => (h * 9 / 16, h),
            Aspect::Portrait => (w, w * 16 / 9),
        };
        cw &= !1;
        ch &= !1;
        ensure!(cw >= 2 && ch >= 2, "Frame is too small for video");
        if cw == w && ch == h {
            return Ok(self);
        }
        let (x, y) = ((w - cw) / 2, (h - ch) / 2);
        let mut rgb = Vec::with_capacity((cw * ch * 3) as usize);
        for row in y..y + ch {
            let start = ((row * w + x) * 3) as usize;
            rgb.extend_from_slice(&self.rgb[start..start + (cw * 3) as usize]);
        }
        Ok(Self {
            width: cw as u16,
            height: ch as u16,
            rgb,
        })
    }
    pub fn nv12(&self) -> Result<Vec<u8>> {
        let (w, h) = (self.width as usize, self.height as usize);
        ensure!(
            w >= 2 && h >= 2 && w % 2 == 0 && h % 2 == 0 && self.rgb.len() == w * h * 3,
            "NV12 needs an even-sized RGB frame"
        );
        let mut output = vec![0; w * h * 3 / 2];
        for y in (0..h).step_by(2) {
            for x in (0..w).step_by(2) {
                let (mut u, mut v) = (0, 0);
                for dy in 0..2 {
                    for dx in 0..2 {
                        let at = ((y + dy) * w + x + dx) * 3;
                        let r = i32::from(self.rgb[at]);
                        let g = i32::from(self.rgb[at + 1]);
                        let b = i32::from(self.rgb[at + 2]);
                        // Limited-range BT.709. Average chroma across the whole 2x2 block.
                        output[(y + dy) * w + x + dx] =
                            (((47 * r + 157 * g + 16 * b + 128) >> 8) + 16).clamp(16, 235) as u8;
                        u += -26 * r - 87 * g + 113 * b;
                        v += 112 * r - 102 * g - 10 * b;
                    }
                }
                let uv = w * h + y / 2 * w + x;
                output[uv] = (((u + 512) >> 10) + 128).clamp(16, 240) as u8;
                output[uv + 1] = (((v + 512) >> 10) + 128).clamp(16, 240) as u8;
            }
        }
        Ok(output)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Aspect {
    #[default]
    Native,
    Portrait,
    Landscape,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn labelled() -> Frame {
        Frame {
            width: 3,
            height: 2,
            rgb: (1..=6).flat_map(|v| [v, v, v]).collect(),
        }
    }
    fn labels(frame: &Frame) -> Vec<u8> {
        frame.rgb.chunks_exact(3).map(|p| p[0]).collect()
    }
    #[test]
    fn clockwise_rotates_pixels_and_swaps_dimensions() {
        let f = labelled().rotate(90).unwrap();
        assert_eq!((f.width, f.height), (2, 3));
        assert_eq!(labels(&f), [4, 1, 5, 2, 6, 3]);
    }
    #[test]
    fn counterclockwise_and_half_turn_have_correct_corners() {
        assert_eq!(labels(&labelled().rotate(270).unwrap()), [3, 6, 2, 5, 1, 4]);
        assert_eq!(labels(&labelled().rotate(180).unwrap()), [6, 5, 4, 3, 2, 1]);
    }
    #[test]
    fn four_quarter_turns_preserve_every_pixel() {
        let mut f = labelled();
        for _ in 0..4 {
            f = f.rotate(90).unwrap();
        }
        assert_eq!(f.rgb, labelled().rgb);
        assert_eq!((f.width, f.height), (3, 2));
    }
    #[test]
    fn encoded_portrait_has_portrait_dimensions_without_exif_rotation() {
        let f = labelled().rotate(90).unwrap();
        let decoded = Frame::decode(&f.jpeg().unwrap()).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 3));
    }
    #[test]
    fn rejects_corrupt_jpeg_and_invalid_rotation() {
        assert!(Frame::decode(&[0xff, 0xd8, 0xff, 0xd9]).is_err());
        assert!(labelled().rotate(45).is_err());
    }
    #[test]
    fn nv12_has_limited_range_neutral_black_and_white() {
        for (rgb, y) in [(0, 16), (255, 235)] {
            let f = Frame {
                width: 2,
                height: 2,
                rgb: vec![rgb; 12],
            };
            assert_eq!(f.nv12().unwrap(), [y, y, y, y, 128, 128]);
        }
        assert!(labelled().nv12().is_err());
    }
    #[test]
    fn crop_preserves_pixels_and_produces_even_video_dimensions() {
        let f = Frame {
            width: 8,
            height: 4,
            rgb: (0..32).flat_map(|v| [v; 3]).collect(),
        }
        .crop(Aspect::Portrait)
        .unwrap();
        assert_eq!((f.width, f.height), (2, 4));
        assert_eq!(labels(&f), [3, 4, 11, 12, 19, 20, 27, 28]);
    }
}
