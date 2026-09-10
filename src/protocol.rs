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

#[cfg(test)]
mod tests {
    use super::*;
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
