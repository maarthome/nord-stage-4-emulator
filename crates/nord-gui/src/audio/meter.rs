//! Lock-free audio callback meter (spec `docs/specs/gui-audio.md`).
//!
//! The audio callback is the single writer; the GUI reads [`MeterSnapshot`]s and asks for a reset.
//! Everything is atomics: no allocation, lock or I/O on the audio thread
//! (`.agents/rules/dsp-and-realtime.md`). A block counts as *late* when its callback took longer
//! than the audio it produced (`frames / sample_rate`), the point where a device buffer of that
//! size would run dry unless the driver holds more buffers than one.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

/// Per-block figures the audio callback hands to [`AudioMeter::record`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BlockSample {
    /// Frames in the device buffer of this callback.
    pub frames: u32,
    /// Whole callback: command drain, render, telemetry publish.
    pub total_ns: u64,
    /// Command drain share of `total_ns`.
    pub drain_ns: u64,
    /// Telemetry publish share of `total_ns`.
    pub publish_ns: u64,
    /// Peak `|sample|` written to the device.
    pub peak: f32,
    /// Samples written with `|sample| > 1.0` (above 0 dBFS, clipped by the device).
    pub over_0db: u32,
}

/// Peak / over-0 dBFS accumulator for the samples of one block.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockStats {
    pub peak: f32,
    pub over_0db: u32,
}

impl BlockStats {
    /// Adds one stereo frame.
    #[inline]
    pub fn push(&mut self, l: f32, r: f32) {
        for s in [l.abs(), r.abs()] {
            // `max` ignores NaN, so a NaN sample is counted as over instead of hidden.
            self.peak = self.peak.max(s);
            self.over_0db += u32::from(s > 1.0 || s.is_nan());
        }
    }
}

/// Atomics shared by the audio callback (writer) and the GUI (reader).
#[derive(Debug, Default)]
pub struct AudioMeter {
    sample_rate: AtomicU32,
    frames: AtomicU32,
    blocks: AtomicU64,
    late: AtomicU64,
    total_ns: AtomicU64,
    worst_ns: AtomicU64,
    worst_drain_ns: AtomicU64,
    worst_publish_ns: AtomicU64,
    peak_bits: AtomicU32,
    over_0db: AtomicU64,
    reset_requested: AtomicBool,
}

/// One read of the meter.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MeterSnapshot {
    pub sample_rate: u32,
    /// Frames per callback of the latest block.
    pub frames: u32,
    pub blocks: u64,
    /// Blocks whose callback took longer than `frames / sample_rate`.
    pub late: u64,
    pub mean_ms: f64,
    pub worst_ms: f64,
    pub worst_drain_ms: f64,
    pub worst_publish_ms: f64,
    pub peak: f32,
    pub over_0db: u64,
}

impl MeterSnapshot {
    /// Deadline of the latest block in milliseconds (`0` before the first block).
    pub fn deadline_ms(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            f64::from(self.frames) * 1000.0 / f64::from(self.sample_rate)
        }
    }

    /// Mean callback time as a share of the deadline.
    pub fn load(&self) -> f64 {
        let d = self.deadline_ms();
        if d > 0.0 {
            self.mean_ms / d
        } else {
            0.0
        }
    }

    /// Peak in dBFS (`-inf` for silence).
    pub fn peak_dbfs(&self) -> f32 {
        20.0 * self.peak.log10()
    }

    /// One-line text for the status bar and the exit log.
    pub fn summary(&self) -> String {
        if self.blocks == 0 {
            return "audio: no block yet".into();
        }
        format!(
            "audio: {} frames @ {} Hz ({:.2} ms), {} blocks, late {}, load {:.0}% (mean {:.3} / worst {:.3} ms), peak {:.1} dBFS, over 0 dBFS {}",
            self.frames,
            self.sample_rate,
            self.deadline_ms(),
            self.blocks,
            self.late,
            self.load() * 100.0,
            self.mean_ms,
            self.worst_ms,
            self.peak_dbfs(),
            self.over_0db
        )
    }
}

impl AudioMeter {
    /// Audio thread, once at stream start.
    pub fn set_sample_rate(&self, hz: u32) {
        self.sample_rate.store(hz, Relaxed);
    }

    /// GUI: zero the counters at the next block (the audio thread is the only one that writes).
    pub fn request_reset(&self) {
        self.reset_requested.store(true, Relaxed);
    }

    /// Audio thread, once per callback. Allocation- and lock-free.
    pub fn record(&self, b: BlockSample) {
        if self.reset_requested.swap(false, Relaxed) {
            for a in [
                &self.blocks,
                &self.late,
                &self.total_ns,
                &self.worst_ns,
                &self.worst_drain_ns,
                &self.worst_publish_ns,
                &self.over_0db,
            ] {
                a.store(0, Relaxed);
            }
            self.peak_bits.store(0, Relaxed);
        }
        let hz = u64::from(self.sample_rate.load(Relaxed).max(1));
        let deadline_ns = u64::from(b.frames) * 1_000_000_000 / hz;
        self.frames.store(b.frames, Relaxed);
        self.blocks.fetch_add(1, Relaxed);
        self.total_ns.fetch_add(b.total_ns, Relaxed);
        if b.total_ns > deadline_ns {
            self.late.fetch_add(1, Relaxed);
        }
        self.worst_ns.fetch_max(b.total_ns, Relaxed);
        self.worst_drain_ns.fetch_max(b.drain_ns, Relaxed);
        self.worst_publish_ns.fetch_max(b.publish_ns, Relaxed);
        self.over_0db.fetch_add(u64::from(b.over_0db), Relaxed);
        // Non-negative floats order like their bit patterns.
        self.peak_bits.fetch_max(b.peak.max(0.0).to_bits(), Relaxed);
    }

    /// GUI: a consistent-enough copy of the counters.
    pub fn snapshot(&self) -> MeterSnapshot {
        let blocks = self.blocks.load(Relaxed);
        let ms = |ns: u64| ns as f64 / 1e6;
        MeterSnapshot {
            sample_rate: self.sample_rate.load(Relaxed),
            frames: self.frames.load(Relaxed),
            blocks,
            late: self.late.load(Relaxed),
            mean_ms: ms(self.total_ns.load(Relaxed)) / blocks.max(1) as f64,
            worst_ms: ms(self.worst_ns.load(Relaxed)),
            worst_drain_ms: ms(self.worst_drain_ns.load(Relaxed)),
            worst_publish_ms: ms(self.worst_publish_ns.load(Relaxed)),
            peak: f32::from_bits(self.peak_bits.load(Relaxed)),
            over_0db: self.over_0db.load(Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(total_ns: u64, peak: f32, over: u32) -> BlockSample {
        BlockSample {
            frames: 128,
            total_ns,
            drain_ns: total_ns / 10,
            publish_ns: total_ns / 20,
            peak,
            over_0db: over,
        }
    }

    fn meter() -> AudioMeter {
        let m = AudioMeter::default();
        m.set_sample_rate(48_000);
        m
    }

    #[test]
    fn late_blocks_are_those_over_frames_divided_by_sample_rate() {
        let m = meter();
        // 128 / 48000 = 2.6667 ms.
        m.record(block(2_666_000, 0.5, 0));
        m.record(block(2_667_000, 0.5, 0));
        m.record(block(20_000_000, 0.5, 0));
        let s = m.snapshot();
        assert_eq!((s.blocks, s.late), (3, 2));
        assert!((s.deadline_ms() - 2.6667).abs() < 1e-3);
        assert!((s.worst_ms - 20.0).abs() < 1e-9);
        assert!((s.mean_ms - 8.444).abs() < 1e-2);
        assert!((s.worst_drain_ms - 2.0).abs() < 1e-9);
    }

    #[test]
    fn peak_and_over_0db_accumulate_and_reset_on_request() {
        let m = meter();
        m.record(block(1, 0.5, 0));
        m.record(block(1, 1.25, 3));
        m.record(block(1, 0.9, 2));
        let s = m.snapshot();
        assert_eq!((s.peak, s.over_0db), (1.25, 5));
        assert!((s.peak_dbfs() - 1.938).abs() < 1e-2);
        m.request_reset();
        assert_eq!(m.snapshot().blocks, 3, "reset waits for the audio thread");
        m.record(block(1, 0.1, 0));
        let s = m.snapshot();
        assert_eq!((s.blocks, s.late, s.over_0db), (1, 0, 0));
        assert_eq!(s.peak, 0.1);
    }

    #[test]
    fn block_stats_counts_over_and_nan_samples() {
        let mut st = BlockStats::default();
        st.push(0.5, -0.75);
        st.push(-1.5, 1.0);
        st.push(f32::NAN, 0.0);
        assert_eq!(st.peak, 1.5);
        assert_eq!(st.over_0db, 2);
    }

    #[test]
    fn empty_meter_reports_nothing() {
        let s = AudioMeter::default().snapshot();
        assert_eq!(s.load(), 0.0);
        assert_eq!(s.summary(), "audio: no block yet");
    }

    #[test]
    fn summary_names_the_figures() {
        let m = meter();
        m.record(block(5_000_000, 0.5, 0));
        let t = m.snapshot().summary();
        assert!(t.contains("128 frames @ 48000 Hz"), "{t}");
        assert!(t.contains("late 1"), "{t}");
    }
}
