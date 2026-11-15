//! Firmware voice-gain slew of one piano stroke voice: the SMP record update of
//! [`super::smp_slew`] driven at the SMP update rate, with the initial values the ARM and the SMP
//! command handler leave in the record at note-on and the reset the ARM writes at release.
//!
//! Evidence: `docs/research/smp-slew.md` section 6 (ARM message lists of a note-on and note-off
//! for four sounds at four velocities, replayed into the `sharc-ref` oracle running the SMP image).
//! The type is `Copy` and holds no heap data, so voices can carry it on the audio path.

use super::smp_slew::SlewRecord;

/// VERIFIED (oracle replay of the note-on message list, 4 velocities, `tests::note_on_gains`):
/// the SMP voice-start handler leaves `F-2 = 0.25` (the ARM word `DM 0x9252D`), `F-1 = 0`,
/// `F2 = 0`, `F3 = 0.5`, `F11 = F12 = 0`, `F13 = F14 = F16 = F18 = 1.0` and the multipliers
/// `F19 = F20 = 0x70000000` (0.875); `F15` is the level word (`DM 0x9252A`) and `F17` the rate
/// word (`DM 0x9252C`).
const RAMP: f32 = 0.25;
const RAMP_CEIL: f32 = 0.5;
const GAIN_MUL: i32 = 0x7000_0000;
/// VERIFIED (ARM note-off list, 4 velocities, identical): the release write resets
/// `F11 = F12 = 0`, `F14 = 1.0`, `F16 = 0x3F7FBB6E` of the main voice record.
const RELEASE_Y_STATE: u32 = 0x3F7F_BB6E;
/// One Q31 unit.
const Q31: f32 = 2_147_483_648.0;
/// Below this gain (about -120 dB of full scale) a released voice is done.
const DONE_GAIN: f32 = 1e-6;

/// Rate word (`F17`) the ARM sends for a velocity and a key.
/// EMPIRICAL: `1 - d / 2^24` with `d` interpolated bilinearly (velocity, then key) over the Velvet
/// Grand note-on words of the firmware run (`tools/arm_smp_slew_init.py rates`; velocity 20 / 64 /
/// 88 / 100 / 127 at notes 36 / 60 / 84, each point an exact ARM word: note 60 gives `d = 374 / 314 /
/// 226 / 190 / 154`). Between and outside the points the law is interpolated and clamped; Bright
/// Grand (velocity 20, 64, 100 at notes 36 / 60 / 84) differs from Velvet by at most 15 %, Wurlitzer
/// 1 and Amber Upright (velocity 90, note 60) by a factor 1.5 to 2 (ASSUMED: one table for every
/// sound; what would confirm it: the ARM rate writer, `docs/research/smp-slew.md` s6).
pub(super) fn rate_for(velocity: u8, note: u8) -> f32 {
    const VEL: [f32; 5] = [20.0, 64.0, 88.0, 100.0, 127.0];
    const KEY: [f32; 3] = [36.0, 60.0, 84.0];
    // d[velocity][key]
    const D: [[f32; 3]; 5] = [
        [238.0, 374.0, 790.0],
        [188.0, 314.0, 572.0],
        [162.0, 226.0, 404.0],
        [140.0, 190.0, 404.0],
        [118.0, 154.0, 316.0],
    ];
    fn locate(axis: &[f32], x: f32) -> (usize, f32) {
        let x = x.clamp(axis[0], axis[axis.len() - 1]);
        let i = axis.windows(2).position(|w| x <= w[1]).unwrap_or(0);
        (i, (x - axis[i]) / (axis[i + 1] - axis[i]))
    }
    let (vi, vt) = locate(&VEL, f32::from(velocity));
    let (ki, kt) = locate(&KEY, f32::from(note));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let at = |row: usize| lerp(D[row][ki], D[row][ki + 1], kt);
    let d = lerp(at(vi), at(vi + 1), vt);
    1.0 - d.round() / 16_777_216.0
}

/// The record the SMP voice-start handler leaves for a main voice (before update 1).
pub(super) fn initial_record(level: f32, rate: f32) -> SlewRecord {
    SlewRecord {
        ramp: RAMP,
        ramp_step: 0.0,
        ramp_floor: 0.0,
        ramp_ceil: RAMP_CEIL,
        x_target: 0.0,
        y_target: 0.0,
        x_coef: 1.0,
        y_decay: 1.0,
        x_state: level,
        y_state: 1.0,
        x_rate: rate,
        y_rate: 1.0,
        gain_l_mul: GAIN_MUL,
        gain_r_mul: GAIN_MUL,
    }
}

/// The slew of one voice: record, update clock and the two current gains.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SlewVoice {
    rec: Option<SlewRecord>,
    /// Record updates per output sample: `frame_rate / 2 / sample_rate`.
    step: f64,
    acc: f64,
    left: f32,
    right: f32,
    released: bool,
}

impl SlewVoice {
    /// Starts a slew from the level word (`level_q23 / 2^23`) and the rate word of a note-on.
    /// The first update runs on the first sample (the oracle's first row is the handler's).
    pub(super) fn start(level: f32, rate: f32, sample_rate: f32) -> Self {
        let rec = initial_record(level, rate);
        let mut v = Self {
            rec: Some(rec),
            step: crate::organ::FIRMWARE_FRAME_RATE_HZ / 2.0 / f64::from(sample_rate),
            acc: 1.0,
            left: 0.0,
            right: 0.0,
            released: false,
        };
        v.advance();
        v
    }

    /// The note-off write of the ARM (applied once; later calls do nothing).
    pub(super) fn release(&mut self) {
        if self.released {
            return;
        }
        if let Some(r) = self.rec.as_mut() {
            r.x_target = 0.0;
            r.y_target = 0.0;
            r.y_decay = 1.0;
            r.y_state = f32::from_bits(RELEASE_Y_STATE);
            self.released = true;
        }
    }

    pub(super) fn is_active(&self) -> bool {
        self.rec.is_some()
    }

    pub(super) fn is_done(&self) -> bool {
        self.released && self.left.max(self.right) < DONE_GAIN
    }

    fn advance(&mut self) {
        if let Some(r) = self.rec.as_mut() {
            let g = r.step();
            self.left = g.left as f32 / Q31;
            self.right = g.right as f32 / Q31;
        }
    }

    /// Gains `(left, right)` for the current output sample, then moves the update clock.
    pub(super) fn tick(&mut self) -> (f32, f32) {
        self.acc += self.step;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.advance();
        }
        (self.left, self.right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(level word, rate word, left gain after update n = 1..=8)` of the oracle replay of a
    /// Velvet Grand note-on (note 60), velocities 20, 64, 100 and 127 (`sharc-ref`, SMP image,
    /// `docs/research/smp-slew.md` section 6): `F0` read after every second interrupt.
    const NOTE_ON: [(u32, u32, [i32; 8]); 4] = [
        (
            0x3D5C4D00,
            0x3F7FFE8A,
            [
                25265292, 25264729, 25264165, 25263602, 25263038, 25262475, 25261911, 25261348,
            ],
        ),
        (
            0x3EE411F0,
            0x3F7FFF42,
            [
                209252830, 209250450, 209248070, 209245690, 209243310, 209240930, 209238550,
                209236170,
            ],
        ),
        (
            0x3E66B858,
            0x3F7FFEC6,
            [
                105841323, 105839342, 105837361, 105835380, 105833399, 105831418, 105829437,
                105827456,
            ],
        ),
        (
            0x3F28E5A2,
            0x3F7FFF66,
            [
                309924507, 309921651, 309918795, 309915939, 309913083, 309910227, 309907371,
                309904515,
            ],
        ),
    ];

    #[test]
    fn rate_table_hits_the_oracle_words_and_clamps() {
        for (vel, word) in [
            (20u8, 0x3F7F_FE8Au32),
            (64, 0x3F7F_FEC6),
            (100, 0x3F7F_FF42),
            (127, 0x3F7F_FF66),
        ] {
            assert_eq!(rate_for(vel, 60).to_bits(), word, "velocity {vel}");
        }
        assert_eq!(rate_for(1, 60).to_bits(), rate_for(20, 60).to_bits());
        assert_eq!(rate_for(100, 0).to_bits(), rate_for(100, 36).to_bits());
        let mid = rate_for(80, 60);
        assert!(mid > rate_for(64, 60) && mid < rate_for(100, 60));
    }

    /// Firmware run (`tools/arm_smp_slew_init.py rates`, Velvet Grand, velocity 88): the rate word
    /// `d = 1 - word` of notes 36 / 60 / 84 is 162 / 226 / 404, notes between them are interpolated
    /// (the run gave 190 at note 48 and 298 at note 72: the interpolation lands within 8 %).
    #[test]
    fn rate_follows_the_key_and_is_faster_for_high_notes() {
        let d = |v: u8, n: u8| ((1.0 - rate_for(v, n)) * 16_777_216.0).round();
        assert_eq!((d(88, 36), d(88, 60), d(88, 84)), (162.0, 226.0, 404.0));
        assert!((d(88, 48) - 190.0).abs() / 190.0 < 0.08, "{}", d(88, 48));
        assert!((d(88, 72) - 298.0).abs() / 298.0 < 0.08, "{}", d(88, 72));
        assert!(d(100, 84) > d(100, 60) && d(100, 60) > d(100, 36));
    }

    #[test]
    fn note_on_gains() {
        for (level, rate, want) in NOTE_ON {
            let mut r = initial_record(f32::from_bits(level), f32::from_bits(rate));
            for (n, &w) in want.iter().enumerate() {
                assert_eq!(r.step().left, w, "level {level:#x} update {}", n + 1);
            }
        }
    }

    /// Oracle: a held main voice (Velvet Grand, velocity 100) after the note-off write
    /// (`F16 = 0x3F7FBB6E` poked, `sharc-ref`): `F0` at the next five updates.
    #[test]
    fn release_gains() {
        let mut r = initial_record(f32::from_bits(0x3EE4_0E9E), f32::from_bits(0x3F7F_FF42));
        r.x_coef = f32::from_bits(RELEASE_Y_STATE); // `F13` after the first update past the write
        r.y_state = f32::from_bits(RELEASE_Y_STATE);
        let want = [
            0x0C75_6C32,
            0x0C72_0C9B,
            0x0C6E_ADF3,
            0x0C6B_5038,
            0x0C67_F36C,
        ];
        // One of the five is +1 LSB in the oracle (`q` one above the model: the rounding of the
        // 40-bit product by `FIX`, open, `docs/research/smp-slew.md` section 6).
        let mut exact = 0;
        for (n, w) in want.into_iter().enumerate() {
            let got = r.step().left;
            assert!(
                (got - w).abs() <= 1,
                "release update {}: {got} vs {w}",
                n + 1
            );
            exact += i32::from(got == w);
        }
        assert_eq!(exact, 4);
    }

    #[test]
    fn hold_gain_is_a_fifth_of_the_level_and_release_decays() {
        let level = 0.4454;
        let mut v = SlewVoice::start(level, f32::from_bits(0x3F7F_FF42), 48_000.0);
        let (l, r) = v.tick();
        assert!((l - 0.21875 * level).abs() < 1e-3 && l == r);
        for _ in 0..4800 {
            v.tick();
        }
        let hold = v.tick().0;
        v.release();
        v.release();
        for _ in 0..4800 {
            v.tick();
        }
        let after = v.tick().0;
        assert!(after < 0.5 * hold, "release must decay: {hold} -> {after}");
        assert!(!v.is_done() || after < DONE_GAIN);
    }

    #[test]
    fn done_after_a_long_release_and_inactive_default() {
        assert!(!SlewVoice::default().is_active());
        let mut v = SlewVoice::start(0.5, f32::from_bits(0x3F7F_FF42), 48_000.0);
        v.release();
        for _ in 0..48_000 * 40 {
            v.tick();
        }
        assert!(v.is_done());
    }
}
