//! Canon EOS packet layouts documented by the libgphoto2 project.
//! See docs/protocol-sources.md for provenance and links.
use anyhow::{Result, bail, ensure};

pub fn word(data: &[u8], offset: usize) -> Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| anyhow::anyhow!("Offset overflow"))?;
    let bytes = data
        .get(offset..end)
        .ok_or_else(|| anyhow::anyhow!("Truncated Canon packet"))?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

pub fn records(mut data: &[u8]) -> Result<Vec<(u32, &[u8])>> {
    let mut result = Vec::new();
    while !data.is_empty() {
        let length = word(data, 0)? as usize;
        let kind = word(data, 4)?;
        ensure!(
            length >= 8 && length <= data.len(),
            "Invalid Canon record length {length} (remaining {})",
            data.len()
        );
        if kind == 0 {
            ensure!(
                length == 8 && length == data.len(),
                "Invalid Canon terminator"
            );
            break;
        }
        result.push((kind, &data[8..length]));
        data = &data[length..];
    }
    Ok(result)
}

pub fn preview_jpeg(data: &[u8]) -> Result<&[u8]> {
    for (kind, bytes) in records(data)? {
        if kind == 1 || kind == 11 {
            ensure!(
                bytes.starts_with(&[0xff, 0xd8]),
                "Canon preview is not a JPEG"
            );
            return Ok(bytes);
        }
    }
    bail!("No JPEG preview in Canon response (movie/raw payloads are not implemented)")
}

pub fn property_packet(code: u32, value: u32) -> [u8; 12] {
    let mut data = [0; 12];
    data[..4].copy_from_slice(&12u32.to_le_bytes());
    data[4..8].copy_from_slice(&code.to_le_bytes());
    data[8..].copy_from_slice(&value.to_le_bytes());
    data
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct FocusPoint {
    /// Normalized sensor coordinates, before display rotation/crop.
    pub rect: [f32; 4],
    pub selected: bool,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct FocusInfo {
    pub points: Vec<FocusPoint>,
}

/// Canon FocusInfoEx (0xD1D3), as documented by libgphoto2 ptp-pack.c.
/// Selection bits are NOT proof of autofocus lock.
pub fn focus_info(data: &[u8]) -> Result<Option<FocusInfo>> {
    let size = word(data, 0)? as usize;
    ensure!(size >= 20 && size <= data.len(), "Invalid focus info size");
    let data = &data[..size];
    let u16_at = |at| u16::from_le_bytes([data[at], data[at + 1]]);
    let count = usize::from(u16_at(8));
    let used = usize::from(u16_at(10));
    if count == 0 || used == 0 {
        return Ok(None);
    }
    ensure!(
        used <= count && count <= 4096 && 20 + count * 8 + count.div_ceil(8) <= size,
        "Truncated focus point arrays"
    );
    let (width, height) = (f32::from(u16_at(12)), f32::from(u16_at(14)));
    ensure!(
        width > 0.0 && height > 0.0,
        "Invalid focus coordinate dimensions"
    );
    let mut points = Vec::with_capacity(used);
    for i in 0..used {
        let h = f32::from(u16_at(20 + i * 2));
        let w = f32::from(u16_at(20 + count * 2 + i * 2));
        let x = f32::from(u16_at(20 + count * 4 + i * 2) as i16) + width / 2.0;
        let y = f32::from(u16_at(20 + count * 6 + i * 2) as i16) + height / 2.0;
        if w == 0.0 || h == 0.0 || w > width || h > height {
            continue;
        }
        points.push(FocusPoint {
            rect: [
                (x - w / 2.0) / width,
                (y - h / 2.0) / height,
                (x + w / 2.0) / width,
                (y + h / 2.0) / height,
            ],
            selected: data[20 + count * 8 + i / 8] & (1 << (i % 8)) != 0,
        });
    }
    Ok(Some(FocusInfo { points }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn focus_packet() -> Vec<u8> {
        let mut data = vec![0; 29];
        data[..4].copy_from_slice(&29u32.to_le_bytes());
        for (at, value) in [
            (8, 1u16),
            (10, 1),
            (12, 1000),
            (14, 800),
            (20, 80),
            (22, 100),
            (24, (-200i16) as u16),
            (26, 100),
        ] {
            data[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
        data[28] = 1;
        data
    }
    #[test]
    fn focus_points_decode_signed_offsets_and_selection() {
        let info = focus_info(&focus_packet()).unwrap().unwrap();
        assert_eq!(info.points.len(), 1);
        assert!(info.points[0].selected);
        assert_eq!(info.points[0].rect, [0.25, 0.575, 0.35, 0.675]);
    }
    #[test]
    fn malformed_focus_packets_never_panic() {
        let packet = focus_packet();
        for end in 0..packet.len() {
            assert!(focus_info(&packet[..end]).is_err());
        }
        let mut invalid = packet.clone();
        invalid[10..12].copy_from_slice(&2u16.to_le_bytes());
        assert!(focus_info(&invalid).is_err());
        invalid = packet.clone();
        invalid[12..14].fill(0);
        assert!(focus_info(&invalid).is_err());
        invalid = packet;
        invalid[10..12].fill(0);
        assert!(focus_info(&invalid).unwrap().is_none());
    }
    fn record(kind: u32, body: &[u8]) -> Vec<u8> {
        [
            (body.len() as u32 + 8).to_le_bytes().as_slice(),
            &kind.to_le_bytes(),
            body,
        ]
        .concat()
    }
    #[test]
    fn jpeg_after_unknown_metadata() {
        let jpeg = [0xff, 0xd8, 0xff, 0xd9];
        let data = [record(7, &[0, 0]), record(1, &jpeg)].concat();
        assert_eq!(preview_jpeg(&data).unwrap(), &jpeg);
    }
    #[test]
    fn rejects_bad_lengths_without_looping_or_panicking() {
        for length in [0u32, 1, 7, 9, u32::MAX] {
            let data = [length.to_le_bytes(), 1u32.to_le_bytes()].concat();
            assert!(records(&data).is_err());
        }
        for length in 1..8 {
            assert!(records(&[0; 8][..length]).is_err());
        }
        assert!(word(&[], usize::MAX).is_err());
    }
    #[test]
    fn accepts_only_final_empty_terminator() {
        assert!(records(&record(0, &[])).unwrap().is_empty());
        assert!(records(&[record(0, &[]), record(1, &[0])].concat()).is_err());
    }
    #[test]
    fn refuses_non_jpeg_or_raw_payloads() {
        assert!(preview_jpeg(&record(1, &[1, 2])).is_err());
        assert!(preview_jpeg(&record(9, &[0xff, 0xd8])).is_err());
    }
    #[test]
    fn validates_trailing_records_even_after_jpeg() {
        assert!(preview_jpeg(&[record(1, &[0xff, 0xd8]), vec![0]].concat()).is_err());
    }
}
