//! Bounded EBML metadata inspection (RFC 8794 / RFC 9559). Codec identifiers
//! are read from TrackEntry, never searched in compressed picture bytes.
//! This is an admission check, not a demuxer; playback still validates data.
pub const HEADER_LIMIT: usize = 1024 * 1024;
#[derive(Default)]
struct Header {
    doc: Option<String>,
    tracks: Vec<(u64, String)>,
    elements: usize,
}
fn vint(data: &[u8], offset: &mut usize, id: bool) -> Result<(u64, bool), String> {
    let first = *data.get(*offset).ok_or("Truncated WebM element")?;
    if first == 0 {
        return Err("Invalid WebM variable-length integer".into());
    }
    let length = first.leading_zeros() as usize + 1;
    if length > if id { 4 } else { 8 } {
        return Err("Invalid WebM element integer length".into());
    }
    let end = offset.checked_add(length).ok_or("WebM integer overflow")?;
    let bytes = data.get(*offset..end).ok_or("Truncated WebM integer")?;
    let mut value = u64::from(if id {
        first
    } else {
        first & ((1u8 << (8 - length)) - 1)
    });
    for byte in &bytes[1..] {
        value = (value << 8) | u64::from(*byte);
    }
    *offset = end;
    Ok((value, !id && value == ((1u64 << (7 * length)) - 1)))
}
fn walk(
    data: &[u8],
    header: &mut Header,
    depth: usize,
    parent: u64,
    track: Option<&mut (u64, String)>,
) -> Result<(), String> {
    if depth > 8 {
        return Err("WebM header nesting limit exceeded".into());
    }
    let mut offset = 0;
    let mut track = track;
    while offset < data.len() {
        header.elements += 1;
        if header.elements > 512 {
            return Err("WebM header element limit exceeded".into());
        }
        let (id, _) = vint(data, &mut offset, true)?;
        let (size, unknown) = vint(data, &mut offset, false)?;
        let bounded = usize::try_from(size)
            .ok()
            .and_then(|size| offset.checked_add(size))
            .unwrap_or(usize::MAX);
        let end = if unknown {
            data.len()
        } else {
            bounded.min(data.len())
        };
        let bytes = &data[offset..end];
        match id {
            0x1A45DFA3 | 0x18538067 | 0x1654AE6B => {
                if !matches!(
                    (parent, id),
                    (0, 0x1A45DFA3 | 0x18538067) | (0x18538067, 0x1654AE6B)
                ) {
                    return Err("Invalid WebM metadata structure".into());
                }
                walk(bytes, header, depth + 1, id, None)?;
            }
            0xAE => {
                if parent != 0x1654AE6B {
                    return Err("WebM track entry is outside Tracks".into());
                }
                if unknown || bounded > data.len() {
                    return Err("WebM track metadata exceeds the 1 MiB inspection limit".into());
                }
                let mut entry = (0, String::new());
                walk(bytes, header, depth + 1, id, Some(&mut entry))?;
                header.tracks.push(entry);
            }
            0x4282 => {
                if parent != 0x1A45DFA3 || unknown || bytes.len() > 16 || header.doc.is_some() {
                    return Err("Invalid WebM document type".into());
                }
                header.doc = Some(
                    std::str::from_utf8(bytes)
                        .map_err(|_| "Invalid WebM document type")?
                        .into(),
                );
            }
            0x83 => {
                if let Some(entry) = track.as_deref_mut() {
                    if unknown || bytes.len() > 8 {
                        return Err("Invalid WebM track type".into());
                    }
                    entry.0 = bytes
                        .iter()
                        .fold(0, |value, byte| (value << 8) | u64::from(*byte));
                }
            }
            0x86 => {
                if let Some(entry) = track.as_deref_mut() {
                    if unknown || bytes.len() > 128 {
                        return Err("Invalid WebM codec identifier".into());
                    }
                    entry.1 = std::str::from_utf8(bytes)
                        .map_err(|_| "Invalid WebM codec identifier")?
                        .into();
                }
            }
            _ => {} // Never recurse into Cluster, codec-private data or images.
        }
        if unknown || bounded >= data.len() {
            break;
        }
        offset = bounded;
    }
    Ok(())
}
pub fn validate_header(data: &[u8], _mask: bool) -> Result<(), String> {
    if data.is_empty() || data.len() > HEADER_LIMIT {
        return Err("Video header must contain at most 1 MiB".into());
    }
    if !data.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return Err("Missing WebM EBML header".into());
    }
    let mut header = Header::default();
    walk(data, &mut header, 0, 0, None)?;
    if header.doc.as_deref() != Some("webm") {
        return Err("Only WebM containers are supported".into());
    }
    let mut video = 0;
    let mut audio = 0;
    for (kind, codec) in header.tracks {
        match kind {
            1 => {
                if codec != "V_VP8" {
                    return Err("Only validated VP8 video is supported".into());
                }
                video += 1;
            }
            2 => {
                if codec != "A_VORBIS" {
                    return Err("Only validated Vorbis video audio is supported".into());
                }
                audio += 1;
            }
            _ => return Err("Unsupported embedded video track; use RVN subtitle cues".into()),
        }
    }
    if video != 1 || audio > 1 {
        return Err("Video needs one VP8 picture stream and at most one soundtrack".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_webm_is_accepted_and_other_codecs_or_missing_metadata_are_rejected() {
        let fixture = include_bytes!("../../examples/videos/assets/clip.webm");
        validate_header(fixture, false).unwrap();
        let mut other = fixture.to_vec();
        let index = other
            .windows(5)
            .position(|bytes| bytes == b"V_VP8")
            .unwrap();
        other[index + 4] = b'9';
        assert!(validate_header(&other, false).unwrap_err().contains("VP8"));
        let mut other = fixture.to_vec();
        let index = other
            .windows(8)
            .position(|bytes| bytes == b"A_VORBIS")
            .unwrap();
        other[index..index + 8].copy_from_slice(b"A_OPUS__");
        assert!(validate_header(&other, false).is_err());
        assert!(validate_header(&other, true).is_err());
        for length in 0..256 {
            let _ = validate_header(&fixture[..length], false);
        }
        for byte in 0..=255 {
            let _ = validate_header(&[byte; 128], false);
        }
        assert!(validate_header(&vec![0; HEADER_LIMIT + 1], false).is_err());
    }
}
