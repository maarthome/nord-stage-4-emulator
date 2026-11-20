//! Reader for an LV2 `atom:Sequence` port buffer (`lv2/atom/atom.h`).
//!
//! Layout (VERIFIED against LV2 1.18 `atom.h`):
//! `LV2_Atom { u32 size; u32 type }`, then `LV2_Atom_Sequence_Body { u32 unit; u32 pad }`, then
//! events `LV2_Atom_Event { i64 frames; LV2_Atom body }` + `body.size` bytes, each padded to 8.
//! `size` counts everything after the 8-byte atom header. Works on a byte slice so it is safe and
//! testable; no allocation, so it is fine on the audio thread.

const ATOM_HEADER: usize = 8;
const SEQ_HEADER: usize = ATOM_HEADER + 8;
const EVENT_HEADER: usize = 16;

fn read_u32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_ne_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn read_i64(b: &[u8], at: usize) -> Option<i64> {
    Some(i64::from_ne_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// Total byte length (header included) of the sequence atom starting at `buf`, from its header.
pub fn sequence_len(header: &[u8]) -> Option<usize> {
    Some(ATOM_HEADER + read_u32(header, 0)? as usize)
}

/// Calls `f(frame, type, data)` for each event of the sequence, in buffer order.
/// Truncated or malformed trailing events are ignored instead of read out of bounds.
pub fn for_each_event(seq: &[u8], mut f: impl FnMut(i64, u32, &[u8])) {
    let Some(total) = sequence_len(seq) else {
        return;
    };
    let end = total.min(seq.len());
    let mut at = SEQ_HEADER;
    while at + EVENT_HEADER <= end {
        let (Some(frames), Some(size), Some(ty)) = (
            read_i64(seq, at),
            read_u32(seq, at + 8),
            read_u32(seq, at + 12),
        ) else {
            return;
        };
        let data_at = at + EVENT_HEADER;
        let data_end = data_at + size as usize;
        if data_end > end {
            return;
        }
        let data = &seq[data_at..data_end];
        f(frames, ty, data);
        at = data_at + (size as usize).next_multiple_of(8);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds a sequence buffer like a host would (frame-time units, unit = 0).
    pub(crate) fn build_sequence(seq_type: u32, events: &[(i64, u32, &[u8])]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&0u32.to_ne_bytes()); // unit
        body.extend_from_slice(&0u32.to_ne_bytes()); // pad
        for (frames, ty, data) in events {
            body.extend_from_slice(&frames.to_ne_bytes());
            body.extend_from_slice(&(data.len() as u32).to_ne_bytes());
            body.extend_from_slice(&ty.to_ne_bytes());
            body.extend_from_slice(data);
            while body.len() % 8 != 0 {
                body.push(0);
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(&(body.len() as u32).to_ne_bytes());
        out.extend_from_slice(&seq_type.to_ne_bytes());
        out.extend_from_slice(&body);
        out
    }

    #[test]
    fn reads_events_in_order_with_padding() {
        let buf = build_sequence(
            9,
            &[
                (0, 5, &[0x90, 60, 100]),
                (17, 5, &[0x80, 60, 0]),
                (17, 6, &[1, 2, 3, 4, 5, 6, 7, 8, 9]),
            ],
        );
        let mut seen = Vec::new();
        for_each_event(&buf, |fr, ty, d| seen.push((fr, ty, d.to_vec())));
        assert_eq!(
            seen,
            vec![
                (0, 5, vec![0x90, 60, 100]),
                (17, 5, vec![0x80, 60, 0]),
                (17, 6, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]),
            ]
        );
    }

    #[test]
    fn empty_and_truncated_buffers_are_safe() {
        let mut n = 0;
        for_each_event(&[], |_, _, _| n += 1);
        for_each_event(&build_sequence(9, &[]), |_, _, _| n += 1);
        let mut buf = build_sequence(9, &[(0, 5, &[0x90, 60, 100])]);
        buf.truncate(buf.len() - 6); // cut into the event payload
        for_each_event(&buf, |_, _, _| n += 1);
        assert_eq!(n, 0);
    }

    #[test]
    fn header_size_bounds_iteration_not_buffer_capacity() {
        // A host hands the full port capacity; only `size` bytes are valid.
        let mut buf = build_sequence(9, &[(3, 5, &[0x90, 61, 90])]);
        buf.extend_from_slice(&[0xAB; 64]);
        let mut n = 0;
        for_each_event(&buf, |_, _, _| n += 1);
        assert_eq!(n, 1);
        assert_eq!(sequence_len(&buf), Some(buf.len() - 64));
    }
}
