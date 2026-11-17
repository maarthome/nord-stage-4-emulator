//! Minimal float WAV helpers for the evaluation binaries (local files only).

/// Reads a RIFF WAV (float 32 or PCM 16, any channel count): per-channel samples and the rate.
pub fn read_wav(bytes: &[u8]) -> Result<(Vec<Vec<f32>>, u32), String> {
    let bad = |s: &str| format!("bad WAV: {s}");
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(bad("no RIFF/WAVE header"));
    }
    let (mut pos, mut fmt, mut data) = (12usize, None, None);
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let body = bytes
            .get(pos + 8..(pos + 8 + len).min(bytes.len()))
            .unwrap_or(&[]);
        if id == b"fmt " && body.len() >= 16 {
            let u16_at = |o: usize| u16::from_le_bytes([body[o], body[o + 1]]);
            let rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            fmt = Some((
                u16_at(0),
                usize::from(u16_at(2)).max(1),
                rate,
                usize::from(u16_at(14)),
            ));
        } else if id == b"data" {
            data = Some(body);
        }
        pos += 8 + len + (len & 1);
    }
    let ((tag, ch, rate, bits), data) = fmt.zip(data).ok_or_else(|| bad("missing fmt or data"))?;
    let step = bits / 8;
    if step == 0 {
        return Err(bad("zero sample size"));
    }
    let frames = data.len() / (step * ch);
    let mut out = vec![Vec::with_capacity(frames); ch];
    for f in 0..frames {
        for (c, o) in out.iter_mut().enumerate() {
            let b = &data[(f * ch + c) * step..][..step];
            o.push(match (tag, bits) {
                (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                (1, 16) => f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0,
                _ => return Err(bad("only PCM16 and float32 are supported")),
            });
        }
    }
    Ok((out, rate))
}

/// Float 32 WAV bytes of `channels` at `rate`.
pub fn write_wav(channels: &[Vec<f32>], rate: u32) -> Vec<u8> {
    let n = channels.iter().map(Vec::len).max().unwrap_or(0);
    let ch = channels.len().max(1) as u16;
    let data_len = (n * usize::from(ch) * 4) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(&ch.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(ch) * 4).to_le_bytes());
    out.extend_from_slice(&(ch * 4).to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..n {
        for c in channels {
            out.extend_from_slice(&c.get(i).copied().unwrap_or(0.0).to_le_bytes());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_wav_round_trips_and_junk_is_rejected() {
        let ch = vec![vec![0.5, -0.25, 1.0], vec![0.0, 0.125, -1.0]];
        let (back, rate) = read_wav(&write_wav(&ch, 43_402)).unwrap();
        assert_eq!((back, rate), (ch, 43_402));
        assert!(read_wav(b"nope").is_err());
        assert!(read_wav(&write_wav(&[vec![0.0]], 8)[..30]).is_err());
    }

    #[test]
    fn pcm16_is_scaled_to_unit_range() {
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF\0\0\0\0WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        for v in [1u16, 1] {
            w.extend_from_slice(&v.to_le_bytes());
        }
        w.extend_from_slice(&8000u32.to_le_bytes());
        w.extend_from_slice(&16000u32.to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&4u32.to_le_bytes());
        w.extend_from_slice(&16384i16.to_le_bytes());
        w.extend_from_slice(&(-32768i16).to_le_bytes());
        assert_eq!(read_wav(&w).unwrap(), (vec![vec![0.5, -1.0]], 8000));
    }
}
