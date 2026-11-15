//! External-port DMA stream model for SY sample voice staging buffers.
//!
//! ```text
//! stream VOICE path=FILE [skip=WORDS]
//!                     ideal external-port DMA model: keeps the 128-word circular staging buffer
//!                     [DM(rec + 0x2F), +0x80) filled ahead of the read pointer DM(rec + 0x1C) with
//!                     consecutive little-endian 32-bit words from FILE, applied before each `irq`.
//!                     Stops writing once the end of FILE is reached (no wrap).
//! ```
//!
//! Provenance:
//! - Staging buffer base `DM(rec + 0x2F)` (`0x93FF7` for voice 0 after prepare) and length
//!   `L1 = 0x80`: `VERIFIED` from `sy2.lst` (`0x1C11AF`, `0x1C1228`: `r0=dm(0x9,i3)` with
//!   `i3 = rec + 0x26`) and post-prepare record dump.
//! - Read pointer `DM(rec + 0x1C)`: `VERIFIED` from `sy2.lst` (`0x1C1235`: `r0=dm(0xfffffff6,i3)`,
//!   `0x1C2321`: `dm(0xfffffff6,i3)=r3`).
//! - Ideal DMA refill policy (eagerly filling every consumed slot up to `read_idx` before each
//!   `irq` and stopping at EOF): `ASSUMED` stand-in for the hardware external-port DMA TCBs
//!   (`bank0_CODE` `0xC05038B0 ..`), ignoring SDRAM bus latency and IOP status registers.
use crate::exec::Runner;
use crate::refill::{BASE_FIELD, BUFFER_WORDS};
use crate::script::{parse_number, Cmd, REC_BASE, REC_STRIDE};
use crate::signal::Opts;
use std::path::Path;

/// Offset of the staging buffer read pointer in a voice record (`DM(rec + 0x1C)`, `VERIFIED` from
/// `sy2.lst` `0x1C1235` / `0x1C2321`).
const READ_PTR_FIELD: u32 = 0x1C;

/// Per-voice state for the `stream` DMA model.
#[derive(Debug, Clone)]
pub struct StreamState {
    pub voice: u32,
    pub words: Vec<u32>,
    /// Index of the next unread word in `words`.
    pub file_pos: usize,
    /// Next circular slot (`0..BUFFER_WORDS`) to write; `None` until `DM(rec + 0x2F) != 0`.
    pub write_idx: Option<usize>,
}

impl StreamState {
    /// Loads little-endian 32-bit words from `path`, skipping the first `skip` words.
    pub fn from_file(voice: u32, path: &Path, skip: usize) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("stream: {}: {e}", path.display()))?;
        if bytes.len() % 4 != 0 {
            return Err(format!(
                "stream: {}: byte length {} is not a multiple of 4",
                path.display(),
                bytes.len()
            ));
        }
        let mut words = Vec::with_capacity(bytes.len() / 4);
        for chunk in bytes.chunks(4) {
            words.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Ok(Self {
            voice,
            file_pos: skip.min(words.len()),
            words,
            write_idx: None,
        })
    }
}

/// Parses `stream VOICE path=FILE [skip=WORDS]`; `None` when `cmd` is not `"stream"`.
pub fn parse(cmd: &str, pos: &[String], opts: &Opts) -> Option<Result<Cmd, String>> {
    if cmd != "stream" {
        return None;
    }
    let voice = match pos.first().and_then(|v| parse_number(v)) {
        Some(v) if v < 56 => v as u32,
        _ => return Some(Err("stream: needs a voice 0..55".into())),
    };
    let Some(path) = opts.get("path") else {
        return Some(Err("stream: needs path=FILE".into()));
    };
    let skip = match opts.get("skip") {
        None => 0,
        Some(s) => match parse_number(s) {
            Some(n) => n as usize,
            None => return Some(Err(format!("stream: bad skip {s:?}"))),
        },
    };
    Some(Ok(Cmd::Stream {
        voice,
        path: path.clone(),
        skip,
    }))
}

impl Runner {
    /// Tops up the circular staging buffer `[DM(rec+0x2F), +0x80)` of every streamed voice
    /// ahead of `DM(rec+0x1C)` before each interrupt.
    pub fn run_stream(&mut self) -> Result<(), String> {
        let buf_len = BUFFER_WORDS as usize;
        for st in &mut self.streams {
            let rec = REC_BASE + REC_STRIDE * st.voice;
            let base = self
                .core
                .dm_read(rec + BASE_FIELD)
                .map_err(|a| format!("stream: bad address 0x{a:x}"))?;
            if base == 0 {
                continue;
            }
            let read_ptr = self
                .core
                .dm_read(rec + READ_PTR_FIELD)
                .map_err(|a| format!("stream: bad address 0x{a:x}"))?;
            if read_ptr < base || read_ptr >= base + BUFFER_WORDS {
                continue;
            }
            let read_idx = (read_ptr - base) as usize;
            let (start_idx, count) = match st.write_idx {
                None => (read_idx, buf_len),
                Some(w) if w == read_idx => continue,
                Some(w) => (w, (read_idx + buf_len - w) % buf_len),
            };
            for k in 0..count {
                let Some(&word) = st.words.get(st.file_pos) else {
                    break;
                };
                st.file_pos += 1;
                let slot = ((start_idx + k) % buf_len) as u32;
                self.core
                    .dm_write(base + slot, word)
                    .map_err(|a| format!("stream: bad address 0x{a:x}"))?;
            }
            st.write_idx = Some(read_idx);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_stream_file(name: &str, words: &[u32]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("sharc_ref_stream_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut bytes = Vec::with_capacity(words.len() * 4);
        for &w in words {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn parses_stream_command() {
        let mut opts = Opts::new();
        opts.insert("path".into(), "foo.bin".into());
        opts.insert("skip".into(), "12".into());
        let pos = vec!["0".to_string()];
        assert_eq!(
            parse("stream", &pos, &opts),
            Some(Ok(Cmd::Stream {
                voice: 0,
                path: "foo.bin".into(),
                skip: 12,
            }))
        );
        assert!(parse("other", &pos, &opts).is_none());
        assert!(parse("stream", &["56".into()], &opts).unwrap().is_err());
        assert!(parse("stream", &pos, &Opts::new()).unwrap().is_err());
    }

    #[test]
    fn dma_fills_and_advances_with_read_pointer_wrap_and_eof() {
        let words: Vec<u32> = (100..260).collect(); // 160 words
        let path = temp_stream_file("wrap_eof.bin", &words);
        let st = StreamState::from_file(0, &path, 4).unwrap(); // starts at 104

        let mut r = Runner::new(&[0u8; 16], false);
        r.streams.push(st);

        // Before prepare (base == 0): nothing written.
        r.run_stream().unwrap();
        assert_eq!(r.core.dm_read(0x93FF7).unwrap(), 0);

        // Prepare sets base = 0x93FF7 and read_ptr = 0x93FF7: fills all 128 words (104..232).
        r.core.dm_write(REC_BASE + BASE_FIELD, 0x93FF7).unwrap();
        r.core.dm_write(REC_BASE + READ_PTR_FIELD, 0x93FF7).unwrap();
        r.run_stream().unwrap();
        assert_eq!(r.core.dm_read(0x93FF7).unwrap(), 104);
        assert_eq!(r.core.dm_read(0x93FF7 + 127).unwrap(), 231);

        // Advance read_ptr by 10 words: slots 0..10 get words 232..242.
        r.core
            .dm_write(REC_BASE + READ_PTR_FIELD, 0x93FF7 + 10)
            .unwrap();
        r.run_stream().unwrap();
        assert_eq!(r.core.dm_read(0x93FF7).unwrap(), 232);
        assert_eq!(r.core.dm_read(0x93FF7 + 9).unwrap(), 241);
        assert_eq!(r.core.dm_read(0x93FF7 + 10).unwrap(), 114); // unread slot untouched

        // Advance read_ptr by 30 more words (only 18 words left until EOF at 260): stops at EOF.
        r.core
            .dm_write(REC_BASE + READ_PTR_FIELD, 0x93FF7 + 40)
            .unwrap();
        r.run_stream().unwrap();
        assert_eq!(r.core.dm_read(0x93FF7 + 10).unwrap(), 242);
        assert_eq!(r.core.dm_read(0x93FF7 + 27).unwrap(), 259);
        assert_eq!(r.core.dm_read(0x93FF7 + 28).unwrap(), 132); // untouched past EOF
        let _ = std::fs::remove_file(&path);
    }
}
