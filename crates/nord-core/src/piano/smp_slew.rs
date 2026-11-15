//! Gain slew of the SMP voice records: the per-interrupt update that turns the float fields of a
//! record into the Q31 gains `L`/`R` (fields `0`, `+1`) that the voice loop of
//! [`super::smp_voice`] multiplies into the bus.
//!
//! Source: SMP image of OS v1.66, loop `0x1C06B9..0x1C06E6` (SIMD, 57 records per interrupt: records
//! `0..=56` on one interrupt, `57..=113` on the next, so every record is updated every second
//! interrupt), run on the `sharc-ref` oracle with random and trajectory records
//! (`docs/research/smp-slew.md`). Field numbers are word offsets from the record pointer `I3`
//! ([`super::smp_voice`] section 2). The loop runs the 40-bit extended float of [`super::ext40`].
//!
//! ```text
//! d_x  = x_coef * x_rate                      ; F13 * F17
//! v    = max(x_target + x_state * d_x, 0)     ; F11 + F15 * d_x          -> F15
//! d_y  = y_decay * y_rate                     ; F14 * F18                -> F14
//! u    = max(y_target + y_state * d_y, 0)     ; F12 + F16 * d_y          -> F16 and F13
//! w    = max(clip(ramp + ramp_step, ramp_ceil), ramp_floor)              -> F-2
//! q    = fix31(w * v)
//! L    = (q * gain_l_mul) >> 31 ;  R = (q * gain_r_mul) >> 31            -> F0, F1
//! ```
//!
//! The y lane (`u`) is not a gain: it feeds the next update's `x_coef` (F13 takes `u`), so the
//! x state decays with a time-varying coefficient. Both output gains use the x lane product
//! (the loop copies the x product into the y lane with a register swap).
//!
//! What the ARM puts into the fields (initial values, the table behind `+17`/`+18`, the meaning of
//! the ramp) is not decoded: the reset routine `0xC00B5F20` sets `F14 = 1.0` and
//! `F11 = F12 = 0`, and the ARM writes `0.979` to the rate words. See the research note.

use super::ext40::Ext;

/// The float fields (memory words) and the two Q31 multipliers of one voice record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlewRecord {
    /// `F-2`: ramp value, replaced by the ramp result each update.
    pub ramp: f32,
    /// `F-1`: ramp increment per update.
    pub ramp_step: f32,
    /// `F+2`: ramp floor.
    pub ramp_floor: f32,
    /// `F+3`: ramp ceiling (clip magnitude).
    pub ramp_ceil: f32,
    /// `F11`, `F12`: targets of the x and y lanes.
    pub x_target: f32,
    pub y_target: f32,
    /// `F13`: x coefficient (receives the y state after each update).
    pub x_coef: f32,
    /// `F14`: y decay state (multiplied by `y_rate` each update).
    pub y_decay: f32,
    /// `F15`, `F16`: x and y lane states.
    pub x_state: f32,
    pub y_state: f32,
    /// `F17`, `F18`: x and y lane rates.
    pub x_rate: f32,
    pub y_rate: f32,
    /// `F19`, `F20`: Q31 multipliers of the left and right gain.
    pub gain_l_mul: i32,
    pub gain_r_mul: i32,
}

/// The values the update writes to the record words `0` and `+1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlewGain {
    pub left: i32,
    pub right: i32,
}

fn ext(x: f32) -> Ext {
    Ext::from_bits(x.to_bits())
}

fn store(x: Ext) -> f32 {
    f32::from_bits(x.to_bits())
}

impl SlewRecord {
    /// One update (one visit of the loop), VERIFIED bit for bit against the oracle
    /// (`tests::oracle_records_and_trajectory`). Integer arithmetic only.
    pub fn step(&mut self) -> SlewGain {
        let d_x = ext(self.x_coef).times(ext(self.x_rate));
        let v = ext(self.x_state)
            .times(d_x)
            .plus(ext(self.x_target))
            .max(Ext::ZERO);
        let d_y = ext(self.y_decay).times(ext(self.y_rate));
        let u = ext(self.y_state)
            .times(d_y)
            .plus(ext(self.y_target))
            .max(Ext::ZERO);
        let w = ext(self.ramp)
            .plus(ext(self.ramp_step))
            .clip(ext(self.ramp_ceil))
            .max(ext(self.ramp_floor));
        let q = i64::from(w.times(v).fix_q31());
        self.ramp = store(w);
        self.x_state = store(v);
        self.y_state = store(u);
        self.x_coef = store(u);
        self.y_decay = store(d_y);
        SlewGain {
            left: ((q * i64::from(self.gain_l_mul)) >> 31) as i32,
            right: ((q * i64::from(self.gain_r_mul)) >> 31) as i32,
        }
    }
}

/// Data captured from the `sharc-ref` oracle running the real SMP interrupt (derived numbers only;
/// the generating scenarios are described in `docs/research/smp-slew.md`).
#[cfg(test)]
mod goldens {
    /// Input words `[F-2, F-1, F2, F3, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20]` and the
    /// words after one update `[F-2, F13, F14, F15, F16, F0, F1]` (oracle `sharc-ref`, SMP image).
    pub const CASES: [([u32; 14], [u32; 7]); 11] = [
        (
            [
                0x3EB71CBD, 0x3E45F232, 0x3F2B1B76, 0x3DF35501, 0x3DD73E22, 0x3F083558, 0x3DAF59FF,
                0x3EEC8677, 0x3DEE4F8B, 0x3E0B7214, 0x3EE8160D, 0x3F55E3F4, 0x1FD8BE11, 0x2C93182C,
            ],
            [
                0x3F2B1B76, 0x3F15A9FB, 0x3EC59E7D, 0x3DE07DEB, 0x3F15A9FB, 0x02555073, 0x034409F2,
            ],
        ),
        (
            [
                0x3F1926C5, 0x3EDA8B9E, 0x3F7A39A9, 0x3DC10801, 0x3E3F8806, 0x3E25C9CD, 0x3EAFA543,
                0x3F53482D, 0x3E6302B8, 0x3F1A3EC3, 0x3F282F0B, 0x3ECEBBF2, 0x561C7D94, 0x6B267211,
            ],
            [
                0x3F7A39A9, 0x3EB9B27E, 0x3EAA9F31, 0x3E72B1CB, 0x3EB9B27E, 0x13F2D3E8, 0x18D289D3,
            ],
        ),
        (
            [
                0x3E7B8E80, 0x3F3245F5, 0x3EE994B6, 0x3EB266B4, 0x3EAB681B, 0x3F4DFE3B, 0x3F36CBA2,
                0x3E90541B, 0x3F187FF6, 0x3F0C871D, 0x3F61A229, 0x3F3E337B, 0x34DB2504, 0x5DF25E7E,
            ],
            [
                0x3EE994B6, 0x3F6B6CC8, 0x3E5676E4, 0x3F35ADA9, 0x3F6B6CC8, 0x111CEB1D, 0x1E6AAE4E,
            ],
        ),
        (
            [
                0x3E53ACD6, 0x3EBFF9D7, 0x3F6FC575, 0x3EE6B6CE, 0x3F148626, 0x3F4CB52C, 0x3F53D2D3,
                0x3EBF0918, 0x3EC3ED41, 0x3F059760, 0x3F4E9AA8, 0x3DEC2F69, 0x1BFAF41B, 0x328D5EC0,
            ],
            [
                0x3F6FC575, 0x3F5274C1, 0x3D303FC3, 0x3F55F126, 0x3F5274C1, 0x15E6AD0A, 0x2791A141,
            ],
        ),
        (
            [
                0x3DDC703D, 0x3F376722, 0x3F2A2E86, 0x3F7E5229, 0x3ED53FBC, 0x3F2F6A95, 0x3D924C66,
                0x3EFA2B29, 0x3E56AD6E, 0x3E251C5D, 0x3DD91A0A, 0x3F47A25F, 0x208E3867, 0x6E82B0F3,
            ],
            [
                0x3F52F529, 0x3F3F24F3, 0x3EC31639, 0x3ED60FD1, 0x3F3F24F3, 0x0B375F54, 0x2612EF8A,
            ],
        ),
        (
            [
                0x3E0196E9, 0x3EF415B2, 0x3F126C7D, 0x3F63A392, 0x3EA10625, 0x3EE399A8, 0x3EC81B36,
                0x3F63D5F0, 0x3F75B861, 0x3E46040F, 0x3E5E9FE6, 0x3E8A6C7F, 0x2DDDF4F5, 0x118B484F,
            ],
            [
                0x3F1A7093, 0x3EFB6C4A, 0x3E7663C5, 0x3ECAC813, 0x3EFB6C4A, 0x0AF58CDE, 0x04311E80,
            ],
        ),
        (
            [
                0x3EEC7EA2, 0x3F0C3DCB, 0x3F56E0DA, 0x3EF1EE26, 0x3E7DAC7B, 0x3F02705B, 0x3F1A9998,
                0x3F3C006D, 0x3E338185, 0x3EAF7921, 0x3E30AD64, 0x3F3F6A4B, 0x68C29793, 0x6135BFB1,
            ],
            [
                0x3F56E0DA, 0x3F329DA4, 0x3F0C9260, 0x3E883052, 0x3F329DA4, 0x1763B2D1, 0x15B428B2,
            ],
        ),
        (
            [
                0x3EEC7EA2, 0x3F0C3DCB, 0x3EFAB7E4, 0x3EF1EE26, 0x3E7DAC7B, 0x3F02705B, 0x3F1A9998,
                0x3F3C006D, 0x3E338185, 0x3EAF7921, 0x3E30AD64, 0x3F3F6A4B, 0x68C29793, 0x6135BFB1,
            ],
            [
                0x3EFAB7E4, 0x3F329DA4, 0x3F0C9260, 0x3E883052, 0x3F329DA4, 0x0DA53312, 0x0CA970E8,
            ],
        ),
        (
            [
                0x3EEC7EA2, 0x3F0C3DCB, 0x3F56E0DA, 0x3EF1EE26, 0x3EF13902, 0x3F02705B, 0x3F1A9998,
                0x3F3C006D, 0x3E338185, 0x3EAF7921, 0x3E30AD64, 0x3F3F6A4B, 0x68C29793, 0x6135BFB1,
            ],
            [
                0x3F56E0DA, 0x3F329DA4, 0x3F0C9260, 0x3EFA9317, 0x3F329DA4, 0x2B08CE09, 0x27EECFFD,
            ],
        ),
        (
            [
                0x3EEC7EA2, 0x3F0C3DCB, 0x3F56E0DA, 0x3EF1EE26, 0x3E7DAC7B, 0x3F02705B, 0x3F1A9998,
                0x3F3C006D, 0x3EB22E17, 0x3EAF7921, 0x3E30AD64, 0x3F3F6A4B, 0x68C29793, 0x6135BFB1,
            ],
            [
                0x3F56E0DA, 0x3F329DA4, 0x3F0C9260, 0x3E916709, 0x3F329DA4, 0x18F8CA7D, 0x172C0E56,
            ],
        ),
        (
            [
                0x3EEC7EA2, 0x3F0C3DCB, 0x3F56E0DA, 0x3EF1EE26, 0x3E7DAC7B, 0x3F02705B, 0x3F1A9998,
                0x3F3C006D, 0x3E338185, 0x3EAF7921, 0x3E30AD64, 0x3F3F6A4B, 0x68C29793, 0x6135BFB1,
            ],
            [
                0x3F56E0DA, 0x3F329DA4, 0x3F0C9260, 0x3E883052, 0x3F329DA4, 0x1763B2D1, 0x15B428B2,
            ],
        ),
    ];
    /// Initial words of the trajectory record (`F19 = 0x7FFFFFFF`, `F20 = 0x60000000`).
    pub const TRAJECTORY_INIT: [u32; 14] = [
        0x00000000, 0x3D23D70A, 0x00000000, 0x3F800000, 0x00000000, 0x00000000, 0x3F733333,
        0x3F800000, 0x3F800000, 0x3F666666, 0x3F7A9FBE, 0x3F7A9FBE, 0x7FFFFFFF, 0x60000000,
    ];
    /// Words `left` gain after each of 60 updates (oracle, one capture per update).
    pub const TRAJECTORY_LEFT: [i32; 60] = [
        79890681, 137826893, 170922170, 176790249, 157478105, 121106268, 79721346, 44310479,
        20457858, 7706600, 2324488, 550550, 100376, 13805, 1403, 102, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ];
    /// Words `right` gain after each of 60 updates (oracle, one capture per update).
    pub const TRAJECTORY_RIGHT: [i32; 60] = [
        59918011, 103370170, 128191628, 132592687, 118108579, 90829701, 59791010, 33232860,
        15343394, 5779950, 1743366, 412913, 75282, 10354, 1053, 77, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ];
}

#[cfg(test)]
mod tests {
    use super::goldens::{CASES, TRAJECTORY_INIT, TRAJECTORY_LEFT, TRAJECTORY_RIGHT};
    use super::*;

    fn record(w: &[u32; 14]) -> SlewRecord {
        let f = f32::from_bits;
        SlewRecord {
            ramp: f(w[0]),
            ramp_step: f(w[1]),
            ramp_floor: f(w[2]),
            ramp_ceil: f(w[3]),
            x_target: f(w[4]),
            y_target: f(w[5]),
            x_coef: f(w[6]),
            y_decay: f(w[7]),
            x_state: f(w[8]),
            y_state: f(w[9]),
            x_rate: f(w[10]),
            y_rate: f(w[11]),
            gain_l_mul: w[12] as i32,
            gain_r_mul: w[13] as i32,
        }
    }

    /// Input order `[F-2, F-1, F2, F3, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20]`,
    /// expected `[F-2, F13, F14, F15, F16, F0, F1]` after one update.
    #[test]
    fn oracle_records_and_trajectory() {
        for (n, (input, want)) in CASES.iter().enumerate() {
            let mut r = record(input);
            let g = r.step();
            let got = [
                r.ramp.to_bits(),
                r.x_coef.to_bits(),
                r.y_decay.to_bits(),
                r.x_state.to_bits(),
                r.y_state.to_bits(),
                g.left as u32,
                g.right as u32,
            ];
            assert_eq!(&got, want, "oracle record case {n}");
        }
        // 60 updates of one record, captured every second interrupt.
        let mut r = record(&TRAJECTORY_INIT);
        for (n, (&l, &rt)) in TRAJECTORY_LEFT.iter().zip(&TRAJECTORY_RIGHT).enumerate() {
            let g = r.step();
            assert_eq!((g.left, g.right), (l, rt), "trajectory update {n}");
        }
    }

    #[test]
    fn ramp_saturates_at_its_ceiling_and_floor() {
        let mut r = SlewRecord {
            ramp: 0.9,
            ramp_step: 0.2,
            ramp_floor: 0.1,
            ramp_ceil: 1.0,
            x_target: 1.0,
            y_target: 0.0,
            x_coef: 0.0,
            y_decay: 0.0,
            x_state: 0.0,
            y_state: 0.0,
            x_rate: 0.0,
            y_rate: 0.0,
            gain_l_mul: i32::MAX,
            gain_r_mul: 0,
        };
        let g = r.step();
        assert_eq!(r.ramp, 1.0);
        assert_eq!(g.left, i32::MAX - 1); // w * v = 1.0 -> q saturates, (q * mul) >> 31 floors
        assert_eq!(g.right, 0);
        r.ramp = -2.0;
        r.ramp_step = -1.0;
        r.step();
        assert_eq!(r.ramp, 0.1f32); // floor wins over the clipped negative ramp
    }

    #[test]
    fn target_with_zero_state_is_reached_in_one_update() {
        let mut r = SlewRecord {
            ramp: 1.0,
            ramp_step: 0.0,
            ramp_floor: 0.0,
            ramp_ceil: 1.0,
            x_target: 0.25,
            y_target: 0.0,
            x_coef: 0.5,
            y_decay: 1.0,
            x_state: 0.0,
            y_state: 0.0,
            x_rate: 1.0,
            y_rate: 1.0,
            gain_l_mul: i32::MAX,
            gain_r_mul: 1 << 30,
        };
        let g = r.step();
        assert_eq!(r.x_state, 0.25);
        assert_eq!(g.left, (1 << 29) - 1);
        assert_eq!(g.right, 1 << 28);
    }
}
