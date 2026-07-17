use nodes::NodeCapabilityDeclaredMediaFacts;

const BITRATES_KBPS: [u32; 16] =
    [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0];
const SAMPLE_RATES: [u32; 4] = [44_100, 48_000, 32_000, 0];

pub(super) fn inspect(bytes: &[u8]) -> Result<NodeCapabilityDeclaredMediaFacts, ()> {
    let mut offset = id3_length(bytes)?;
    let mut samples = 0_u64;
    let mut channels = None;
    while offset < bytes.len() {
        if bytes.len() - offset == 128 && &bytes[offset..offset + 3] == b"TAG" {
            offset = bytes.len();
            break;
        }
        let header = frame_header(bytes.get(offset..offset + 4).ok_or(())?)?;
        let end = offset.checked_add(header.length).ok_or(())?;
        if end > bytes.len() {
            return Err(());
        }
        channels.get_or_insert(header.channels);
        if channels != Some(header.channels) {
            return Err(());
        }
        samples = samples.checked_add(1_152).ok_or(())?;
        offset = end;
    }
    if offset != bytes.len() || samples == 0 {
        return Err(());
    }
    let duration_ms = samples.checked_mul(1_000).ok_or(())? / 44_100;
    NodeCapabilityDeclaredMediaFacts::try_audio(duration_ms, 44_100, channels.ok_or(())?)
        .map_err(|_| ())
}

struct FrameHeader {
    length: usize,
    channels: u8,
}

fn frame_header(bytes: &[u8]) -> Result<FrameHeader, ()> {
    let header = u32::from_be_bytes(bytes.try_into().map_err(|_| ())?);
    if header >> 21 != 0x7ff || (header >> 19) & 0b11 != 0b11 || (header >> 17) & 0b11 != 0b01 {
        return Err(());
    }
    let bitrate = BITRATES_KBPS[((header >> 12) & 0x0f) as usize];
    let sample_rate = SAMPLE_RATES[((header >> 10) & 0x03) as usize];
    if bitrate != 128 || sample_rate != 44_100 {
        return Err(());
    }
    let padding = ((header >> 9) & 1) as usize;
    let channels = if (header >> 6) & 0b11 == 0b11 { 1 } else { 2 };
    Ok(FrameHeader {
        length: (144 * bitrate as usize * 1_000 / sample_rate as usize) + padding,
        channels,
    })
}

fn id3_length(bytes: &[u8]) -> Result<usize, ()> {
    if !bytes.starts_with(b"ID3") {
        return Ok(0);
    }
    let size = bytes.get(6..10).ok_or(())?;
    if size.iter().any(|byte| byte & 0x80 != 0) {
        return Err(());
    }
    let body = size.iter().fold(0_usize, |value, byte| (value << 7) | *byte as usize);
    10_usize.checked_add(body).filter(|end| *end <= bytes.len()).ok_or(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_complete_fixed_format_frames_and_rejects_truncation() {
        let mut bytes = frame();
        bytes.extend(frame());
        assert!(inspect(&bytes).is_ok());
        bytes.pop();
        assert!(inspect(&bytes).is_err());
    }

    pub(super) fn frame() -> Vec<u8> {
        let header = [0xff, 0xfb, 0x90, 0x00];
        let mut bytes = vec![0; 417];
        bytes[..4].copy_from_slice(&header);
        bytes
    }
}
