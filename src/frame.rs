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
}
