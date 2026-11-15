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
        self.step_q().1
    }

    /// Like [`Self::step`], also returns `q` (the Q31 product before the gain multipliers).
    pub fn step_q(&mut self) -> (i64, SlewGain) {
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
        let gain = SlewGain {
            left: ((q * i64::from(self.gain_l_mul)) >> 31) as i32,
            right: ((q * i64::from(self.gain_r_mul)) >> 31) as i32,
        };
        (q, gain)
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
    pub const TIE_CASES: [([u32; 14], [u32; 7]); 47] = [
        (
            [
                0x3EF5A32E, 0x3F14EFC8, 0x3F6D9169, 0x3EFC179A, 0x3E66D66C, 0x3F094BD3, 0x3F25FCCB,
                0x3F4DA6E8, 0x3E0EC368, 0x3EAD2CA4, 0x3E0B6780, 0x3F51B49F, 0x60000000, 0x60000000,
            ],
            [
                0x3F6D9169, 0x3F424681, 0x3F28765C, 0x3E737073, 0x3F424681, 0x152DE11B, 0x152DE11B,
            ],
        ),
        (
            [
                0x3F3571BA, 0x3DB7E1E4, 0x3F7BAB60, 0x3F776DD9, 0x3E4C6907, 0x3D8395DF, 0x3F0D4D65,
                0x3DDA433C, 0x3E6C3C0F, 0x3E8F47F5, 0x3DA0EDBE, 0x3EFB41FA, 0x7FFFFFFF, 0x7FFFFFFF,
            ],
            [
                0x3F7BAB60, 0x3DA18F44, 0x3D56382C, 0x3E56A803, 0x3DA18F44, 0x1A60CDD2, 0x1A60CDD2,
            ],
        ),
        (
            [
                0x3EEFDFD4, 0x3F59ADA4, 0x3F0B0D0D, 0x3F2884D9, 0x3EF80B96, 0x3EA0E600, 0x3F7F6E14,
                0x3F7EF3C4, 0x3F5923F3, 0x3F38F076, 0x3EB2F36A, 0x3E894F3B, 0x20000000, 0x20000000,
            ],
            [
                0x3F2884D9, 0x3F01D7FE, 0x3E88BF5B, 0x3F47BF43, 0x3F01D7FE, 0x106FA587, 0x106FA587,
            ],
        ),
        (
            [
                0x3EA63064, 0x3DEF06DF, 0x3F472945, 0x3EDC5ABE, 0x3F75CBC1, 0x3F5ADDA1, 0x3D4EEBA3,
                0x3E7F368E, 0x3F6A2D9A, 0x3EFE33AA, 0x3F7B392A, 0x3EDAE840, 0x50000000, 0x50000000,
            ],
            [
                0x3F472945, 0x3F6868DA, 0x3DDA3BFE, 0x3F80B3E1, 0x3F6868DA, 0x3E945C45, 0x3E945C45,
            ],
        ),
        (
            [
                0x3DF480DF, 0x3F25E229, 0x3F4A2243, 0x3E9CD1A0, 0x3F77436A, 0x3F4527CC, 0x3E25FB78,
                0x3E91716F, 0x3E157F73, 0x3DDAEDB9, 0x3F4EA2B9, 0x3E600B98, 0x10000000, 0x10000000,
            ],
            [
                0x3F4A2243, 0x3F46DB38, 0x3D7E93AE, 0x3F7C273C, 0x3F46DB38, 0x0C718B48, 0x0C718B48,
            ],
        ),
        (
            [
                0x3F14D211, 0x3EF33A3B, 0x3E6CB2A3, 0x3F3ECBF3, 0x3E2489FA, 0x3EE6416B, 0x3E81234B,
                0x3E9CD40A, 0x3F78EE11, 0x3F50308F, 0x3EAD8944, 0x3F63FFCB, 0x78000000, 0x78000000,
            ],
            [
                0x3F3ECBF3, 0x3F2BEC1F, 0x3E8BACB8, 0x3E79A929, 0x3F2BEC1F, 0x15CE2973, 0x15CE2973,
            ],
        ),
        (
            [
                0x3E8016E6, 0x3ED9600C, 0x3F5C959F, 0x3F28E4F9, 0x3E815253, 0x3E9739E9, 0x3F48B7D8,
                0x3EB99A9A, 0x3EA9BB7C, 0x3DF53448, 0x3E0ADDB1, 0x3F1A8567, 0x70000000, 0x70000000,
            ],
            [
                0x3F5C959F, 0x3EA4A3B1, 0x3E600F71, 0x3E935E61, 0x3EA4A3B1, 0x1BC6F557, 0x1BC6F557,
            ],
        ),
        (
            [
                0x3E8FCD2E, 0x3F1F0840, 0x3ECE6598, 0x3EF60A59, 0x3F188925, 0x3F5F89FF, 0x3E650E07,
                0x3E492491, 0x3F69BA89, 0x3F53B07F, 0x3E92F4C3, 0x3E6BD68D, 0x40000000, 0x40000000,
            ],
            [
                0x3EF60A59, 0x3F691DA4, 0x3D394D24, 0x3F278AB7, 0x3F691DA4, 0x1420C240, 0x1420C240,
            ],
        ),
        (
            [
                0x3F40A0C5, 0x3F7181A8, 0x3E72714B, 0x3F73DF80, 0x3EE698C7, 0x3E18371A, 0x3DB1B017,
                0x3F76EC95, 0x3E8D8FAF, 0x3F38275B, 0x3E969883, 0x3F5520D0, 0x60000000, 0x60000000,
            ],
            [
                0x3F73DF80, 0x3F39EEAB, 0x3F4D927E, 0x3EEA3597, 0x3F39EEAB, 0x29D580A4, 0x29D580A4,
            ],
        ),
        (
            [
                0x3F1DDC51, 0x3EA853B3, 0x3E5DDC92, 0x3F3BFD6C, 0x3F14D680, 0x3F5C1A89, 0x3F2232D0,
                0x3EA1E612, 0x3F6BE6E9, 0x3E79A173, 0x3D86A5DE, 0x3E9C8932, 0x7FFFFFFF, 0x7FFFFFFF,
            ],
            [
                0x3F3BFD6C, 0x3F62230F, 0x3DC5FDED, 0x3F1EAA23, 0x3F62230F, 0x3A41AC6A, 0x3A41AC6A,
            ],
        ),
        (
            [
                0x3EF26423, 0x3DDC05BA, 0x3E5EA8E0, 0x3ECCFA2A, 0x3EC9BF56, 0x3F657A07, 0x3F7B4189,
                0x3F2C90DD, 0x3F34E7B4, 0x3F1AEF95, 0x3E3BBAD0, 0x3DAAA717, 0x20000000, 0x20000000,
            ],
            [
                0x3ECCFA2A, 0x3F6E2DE6, 0x3D6611A9, 0x3F056CA5, 0x3F6E2DE6, 0x06AD4F62, 0x06AD4F62,
            ],
        ),
        (
            [
                0x3D893707, 0x3F6A29E6, 0x3F3746A2, 0x3F76F227, 0x3F021469, 0x3F3E7505, 0x3EB4B714,
                0x3F7FD802, 0x3DF8D4DD, 0x3F119C43, 0x3F400A2C, 0x3F67BA47, 0x50000000, 0x50000000,
            ],
            [
                0x3F76F227, 0x3FA11731, 0x3F679613, 0x3F0A50B4, 0x3FA11731, 0x29B1DEC4, 0x29B1DEC4,
            ],
        ),
        (
            [
                0x3F400F51, 0x3F37F004, 0x3F4DB8F3, 0x3F6B5421, 0x3F67E220, 0x3F60A6DE, 0x3EE480D7,
                0x3F4D0EB1, 0x3F5ECBEB, 0x3F181B56, 0x3F24CA58, 0x3ED39129, 0x10000000, 0x10000000,
            ],
            [
                0x3F6B5421, 0x3F897FAD, 0x3EA97762, 0x3F93F1DF, 0x3F897FAD, 0xFFFFFFFF, 0xFFFFFFFF,
            ],
        ),
        (
            [
                0x3F1A81EC, 0x3F20E05E, 0x3E013875, 0x3F284D9A, 0x3F3DE664, 0x3ED68912, 0x3F3F8FB0,
                0x3F1A1676, 0x3EEFDEC2, 0x3F58B10C, 0x3E04B40E, 0x3F434049, 0x78000000, 0x78000000,
            ],
            [
                0x3F284D9A, 0x3F4EBEB5, 0x3EEB0B9A, 0x3F4987C8, 0x3F4EBEB5, 0x3E1B2798, 0x3E1B2798,
            ],
        ),
        (
            [
                0x3DA05BEE, 0x3F1F0857, 0x3F01C4C9, 0x3E89946E, 0x3F223F48, 0x3F6CA82B, 0x3E96092B,
                0x3D78CC71, 0x3EAC05B4, 0x3F31B911, 0x3E7843AF, 0x3E58319F, 0x70000000, 0x70000000,
            ],
            [
                0x3F01C4C9, 0x3F6EEFA2, 0x3C521CB9, 0x3F285BA1, 0x3F6EEFA2, 0x25565200, 0x25565200,
            ],
        ),
        (
            [
                0x3F69124D, 0x3F2D4F40, 0x3EF08E53, 0x3F65AB01, 0x3E744E72, 0x3EEB2FFE, 0x3F50D0FE,
                0x3F6B237B, 0x3F62E1A3, 0x3ED494CD, 0x3F1A9C96, 0x3EB38A09, 0x40000000, 0x40000000,
            ],
            [
                0x3F65AB01, 0x3F17D41D, 0x3EA4E898, 0x3F2CD8C3, 0x3F17D41D, 0x26C45725, 0x26C45725,
            ],
        ),
        (
            [
                0x3E37AC1E, 0x3F058A71, 0x3F5861B4, 0x3F5B3572, 0x3EA03BCB, 0x3E57BA86, 0x3EF4CBC0,
                0x3E9F706F, 0x3E81BA8C, 0x3EE2F653, 0x3F24FA79, 0x3F04E916, 0x60000000, 0x60000000,
            ],
            [
                0x3F5861B4, 0x3E908EFD, 0x3E258E48, 0x3EC834A9, 0x3E908EFD, 0x1FBAA862, 0x1FBAA862,
            ],
        ),
        (
            [
                0x3EB2FF2C, 0x3F58DFA4, 0x3F7BA19B, 0x3EF5AF5B, 0x3F611270, 0x3DB71EAD, 0x3F392398,
                0x3F1790C7, 0x3EAFE991, 0x3F4D4BCC, 0x3D8B9696, 0x3E37629C, 0x7FFFFFFF, 0x7FFFFFFF,
            ],
            [
                0x3F7BA19B, 0x3E32A119, 0x3DD925C0, 0x3F656855, 0x3E32A119, 0x70BF0EE9, 0x70BF0EE9,
            ],
        ),
        (
            [
                0x3EF6D49B, 0x3D968205, 0x3F56934B, 0x3E8D1364, 0x3F25D110, 0x3EF2C4AC, 0x3F2601DF,
                0x3F2C1B41, 0x3F5127F0, 0x3F75E5CB, 0x3F3344C5, 0x3E751E8D, 0x20000000, 0x20000000,
            ],
            [
                0x3F56934B, 0x3F20F4CB, 0x3E24CA9F, 0x3F8265AF, 0x3F20F4CB, 0x1B5301C4, 0x1B5301C4,
            ],
        ),
        (
            [
                0x3F005AC8, 0x3E61069D, 0x3D76B1FD, 0x3EFF4720, 0x3E9E12CD, 0x3EC1C489, 0x3F3662DC,
                0x3F0B5DEB, 0x3F223BD2, 0x3F44B5A2, 0x3ED9019C, 0x3F4D65DE, 0x50000000, 0x50000000,
            ],
            [
                0x3EFF4720, 0x3F36CE20, 0x3EDFA354, 0x3F00067D, 0x3F36CE20, 0x13F2915A, 0x13F2915A,
            ],
        ),
        (
            [
                0x3F693264, 0x3E08099D, 0x3F6F9BF8, 0x3F3C7B6D, 0x3F24EEEE, 0x3F6A1A84, 0x3ED0E07A,
                0x3F1722B5, 0x3F62A69C, 0x3F4E92E4, 0x3F72718C, 0x3EFB25BF, 0x10000000, 0x10000000,
            ],
            [
                0x3F6F9BF8, 0x3F92F678, 0x3E944550, 0x3F7C807C, 0x3F92F678, 0x0EC55CAB, 0x0EC55CAB,
            ],
        ),
        (
            [
                0x3F2B339F, 0x3E7A8570, 0x3F3C5FF0, 0x3F53D257, 0x3E8158F3, 0x3F67AD11, 0x3F7B4185,
                0x3F7A7E5C, 0x3F0F634A, 0x3F4D1E93, 0x3EB570A4, 0x3F6A1C08, 0x78000000, 0x78000000,
            ],
            [
                0x3F53D257, 0x3FCF9C4A, 0x3F6512ED, 0x3EE5171E, 0x3FCF9C4A, 0x2C6D5921, 0x2C6D5921,
            ],
        ),
        (
            [
                0x3F5CED32, 0x3EC31D24, 0x3E03B89F, 0x3EF00DE9, 0x3F0358E4, 0x3D9DACDA, 0x3F519538,
                0x3DE3076F, 0x3F4F536F, 0x3E5B64BE, 0x3EBC8BCE, 0x3F4C6B31, 0x70000000, 0x70000000,
            ],
            [
                0x3EF00DE9, 0x3DC4840B, 0x3DB548FC, 0x3F41DA41, 0x3DC4840B, 0x27C39043, 0x27C39043,
            ],
        ),
        (
            [
                0x3E3BE0EB, 0x3E43D4E2, 0x3F0A6B3A, 0x3F3CC577, 0x3F72CE47, 0x3F049884, 0x3F73A264,
                0x3E06E238, 0x3E854BBD, 0x3F0CE271, 0x3EA6BD10, 0x3F3E0DFF, 0x40000000, 0x40000000,
            ],
            [
                0x3F0A6B3A, 0x3F125F7B, 0x3DC8468A, 0x3F83BB23, 0x3F125F7B, 0x239D06F8, 0x239D06F8,
            ],
        ),
        (
            [
                0x3F282C68, 0x3F0BF0D6, 0x3F59F81E, 0x3F14FC27, 0x3F5A5E0C, 0x3F67CEC5, 0x3E7DC684,
                0x3F5BB523, 0x3F78531F, 0x3F0C4AB0, 0x3F1826A4, 0x3E76B36F, 0x60000000, 0x60000000,
            ],
            [
                0x3F59F81E, 0x3F826853, 0x3E53BA0E, 0x3F7EF1AD, 0x3F826853, 0x5166BB5F, 0x5166BB5F,
            ],
        ),
        (
            [
                0x3F0F21BC, 0x3F072C04, 0x3F1FFDCC, 0x3D9C66B1, 0x3EDC71AC, 0x3F4F9EB0, 0x3F15B06E,
                0x3F043896, 0x3F34D915, 0x3DE69D90, 0x3F0FD0CB, 0x3EE2DC16, 0x7FFFFFFF, 0x7FFFFFFF,
            ],
            [
                0x3F1FFDCC, 0x3F563784, 0x3E6A5740, 0x3F29A0C3, 0x3F563784, 0x3501825B, 0x3501825B,
            ],
        ),
        (
            [
                0x3F758274, 0x3F6D601D, 0x3E9C8BAC, 0x3EFFBEEA, 0x3F532EB1, 0x3F67D0AA, 0x3F00B158,
                0x3EB3E4A9, 0x3E6D71B9, 0x3F231271, 0x3F6DD36A, 0x3E3123DE, 0x20000000, 0x20000000,
            ],
            [
                0x3EFFBEEA, 0x3F71BA04, 0x3D78F49A, 0x3F6EE7B0, 0x3F71BA04, 0x0EEAAF38, 0x0EEAAF38,
            ],
        ),
        (
            [
                0x3F4A52B3, 0x3D92BB7A, 0x3E7006E3, 0x3E882391, 0x3EC670DA, 0x3F2386F5, 0x3E193B8D,
                0x3F3E8DA9, 0x3E2AA4C6, 0x3F08F232, 0x3E937824, 0x3E738CF6, 0x50000000, 0x50000000,
            ],
            [
                0x3E882391, 0x3F3BC594, 0x3E354964, 0x3ECA1E45, 0x3F3BC594, 0x0865B33C, 0x0865B33C,
            ],
        ),
        (
            [
                0x3F0DC816, 0x3EEE0C40, 0x3ED05BAC, 0x3EE2ADC2, 0x3E79E903, 0x3F2655F7, 0x3F281341,
                0x3F0D9A75, 0x3F5BD374, 0x3F219172, 0x3F5D2A41, 0x3E8AC3DA, 0x10000000, 0x10000000,
            ],
            [
                0x3EE2ADC2, 0x3F3E8EA5, 0x3E198330, 0x3F3B29FC, 0x3F3E8EA5, 0x052DD0CF, 0x052DD0CF,
            ],
        ),
        (
            [
                0x3F40F4A5, 0x3F51EA65, 0x3F6854C8, 0x3EB33DB3, 0x3E83B495, 0x3F7F9983, 0x3F64A670,
                0x3E357D41, 0x3E8E04C4, 0x3F3D80DF, 0x3E97D1D1, 0x3E1193B9, 0x78000000, 0x78000000,
            ],
            [
                0x3F6854C8, 0x3F822FF0, 0x3CCE6957, 0x3EA9516F, 0x3F822FF0, 0x2403D091, 0x2403D091,
            ],
        ),
        (
            [
                0x3F572EF2, 0x3EE6AE5E, 0x3F4CE993, 0x3E2DC6A7, 0x3D88F387, 0x3E76AB2D, 0x3F32C16E,
                0x3F6A7227, 0x3F784F42, 0x3E237899, 0x3F07C817, 0x3F452EE3, 0x70000000, 0x70000000,
            ],
            [
                0x3F4CE993, 0x3EB4FD7D, 0x3F3494C8, 0x3EDA2A0B, 0x3EB4FD7D, 0x26331F40, 0x26331F40,
            ],
        ),
        (
            [
                0x3F0713D2, 0x3F338EDA, 0x3E6B10F8, 0x3DEFAA8E, 0x3F12F76D, 0x3F0A015A, 0x3F171E3C,
                0x3E41C4BE, 0x3E66B787, 0x3E79914B, 0x3F5923DF, 0x3F7DA42C, 0x40000000, 0x40000000,
            ],
            [
                0x3E6B10F8, 0x3F15B3E6, 0x3E3FFBB3, 0x3F2FD8AF, 0x3F15B3E6, 0x0A177914, 0x0A177914,
            ],
        ),
        (
            [
                0x3F6E3860, 0x3E0FDB11, 0x3DDEEB9C, 0x3F74341D, 0x3EB8919C, 0x3EFCBAB2, 0x3F0A1E2A,
                0x3EEACAA1, 0x3F1EF235, 0x3D802BE3, 0x3F374A5F, 0x3F5A1FEE, 0x60000000, 0x60000000,
            ],
            [
                0x3F74341D, 0x3F049FEC, 0x3EC80DD5, 0x3F19AEE4, 0x3F049FEC, 0x36F9BDF6, 0x36F9BDF6,
            ],
        ),
        (
            [
                0x3E6389E6, 0x3EF666DA, 0x3F409B28, 0x3EDEBD2B, 0x3F09746D, 0x3D846588, 0x3F6605C9,
                0x3F4FC662, 0x3F382D16, 0x3F5E2146, 0x3F25DDE4, 0x3EDE5A87, 0x7FFFFFFF, 0x7FFFFFFF,
            ],
            [
                0x3F409B28, 0x3EBDB079, 0x3EB47782, 0x3F74AD37, 0x3EBDB079, 0x5C0B1A53, 0x5C0B1A53,
            ],
        ),
        (
            [
                0x3F1E9FBD, 0x3F077169, 0x3F7BC9B0, 0x3F508856, 0x3F41D8B9, 0x3F4A01B6, 0x3F52EBBE,
                0x3EDEE6D4, 0x3F66D6FB, 0x3F62C707, 0x3F35C66B, 0x3F476667, 0x20000000, 0x20000000,
            ],
            [
                0x3F7BC9B0, 0x3F8B741A, 0x3EAD9E98, 0x3FA4725C, 0x3F8B741A, 0xFFFFFFFF, 0xFFFFFFFF,
            ],
        ),
        (
            [
                0x3F46E75B, 0x3EDEF462, 0x3F3C8B70, 0x3DEFAA51, 0x3D760696, 0x3EC694F9, 0x3F282281,
                0x3F248FCE, 0x3E8A7F37, 0x3F728B8D, 0x3F2ECB41, 0x3EBDE9DC, 0x50000000, 0x50000000,
            ],
            [
                0x3F3C8B70, 0x3F1D1F72, 0x3E7428FE, 0x3E39B8B2, 0x3F1D1F72, 0x0AAFAFEC, 0x0AAFAFEC,
            ],
        ),
        (
            [
                0x3F2D4102, 0x3F17521E, 0x3F0E71EA, 0x3ED7193B, 0x3F37581B, 0x3F460E1B, 0x3F7B2837,
                0x3D92CCB2, 0x3F2276E4, 0x3F407994, 0x3E966D31, 0x3EDCEAAB, 0x10000000, 0x10000000,
            ],
            [
                0x3F0E71EA, 0x3F4C020B, 0x3CFD5CF4, 0x3F662C6B, 0x3F4C020B, 0x080130FC, 0x080130FC,
            ],
        ),
        (
            [
                0x3DC891AA, 0x3E715478, 0x3ED05443, 0x3E12F8A5, 0x3F1292F4, 0x3F084D3B, 0x3F7801EE,
                0x3F16EEB0, 0x3F7ECF40, 0x3F27F6F4, 0x3F51AD61, 0x3DFAA59C, 0x78000000, 0x78000000,
            ],
            [
                0x3ED05443, 0x3F146BE1, 0x3D93C6BB, 0x3FAE6157, 0x3F146BE1, 0x4284FA53, 0x4284FA53,
            ],
        ),
        (
            [
                0x3F1E1EF3, 0x3F4572FD, 0x3DBE2C4D, 0x3F6F036F, 0x3E57B8F7, 0x3F054F89, 0x3F216FDB,
                0x3DD84E48, 0x3F72B0C4, 0x3EE63AF9, 0x3F0CE859, 0x3F1E3158, 0x70000000, 0x70000000,
            ],
            [
                0x3F6F036F, 0x3F0CD2E1, 0x3D85AA01, 0x3F0A2B46, 0x3F0CD2E1, 0x38701C7E, 0x38701C7E,
            ],
        ),
        (
            [
                0x3ECB6A75, 0x3EA496C9, 0x3F2C1F85, 0x3F152610, 0x3EA99817, 0x3D81A9A4, 0x3E90C773,
                0x3DB99F02, 0x3E4B873B, 0x3F44545D, 0x3ED7463E, 0x3F67150E, 0x40000000, 0x40000000,
            ],
            [
                0x3F2C1F85, 0x3E0114B0, 0x3DA78DBF, 0x3EB5B175, 0x3E0114B0, 0x0F45333A, 0x0F45333A,
            ],
        ),
        (
            [
                0x3F42CD4A, 0x3DC7FCE2, 0x3F7D44CB, 0x3F727DF5, 0x3EEA8436, 0x3F00F452, 0x3F797977,
                0x3E9023C4, 0x3F0C14BD, 0x3F70BE8A, 0x3F3C91E0, 0x3EFD67D6, 0x60000000, 0x60000000,
            ],
            [
                0x3F7D44CB, 0x3F227F9B, 0x3E0EADCF, 0x3F59CFD3, 0x3F227F9B, 0x50CED8A6, 0x50CED8A6,
            ],
        ),
        (
            [
                0x3F7AD966, 0x3F536BA4, 0x3F1F99E7, 0x3E2329A8, 0x3E795146, 0x3DCBADF9, 0x3F0D3BDE,
                0x3E2C24ED, 0x3EF10255, 0x3F2F3B2D, 0x3EF72E62, 0x3E991989, 0x7FFFFFFF, 0x7FFFFFFF,
            ],
            [
                0x3F1F99E7, 0x3E091306, 0x3D4DE67A, 0x3EBCD9AB, 0x3E091306, 0x1D6F2E7B, 0x1D6F2E7B,
            ],
        ),
        (
            [
                0x3F1A6663, 0x3EE5A908, 0x3F4A0063, 0x3F0DE1E2, 0x3F3F60C7, 0x3E8D9029, 0x3E21F8A9,
                0x3F65E790, 0x3F4B8B3D, 0x3F24CB55, 0x3EC84E9B, 0x3E9DA9D2, 0x20000000, 0x20000000,
            ],
            [
                0x3F4A0063, 0x3EE8B5B0, 0x3E8D9785, 0x3F4BF947, 0x3EE8B5B0, 0x141E6023, 0x141E6023,
            ],
        ),
        (
            [
                0x3F3366C5, 0x3F162E6D, 0x3F1CB204, 0x3F26C524, 0x3E92B1D4, 0x3F7B1170, 0x3F6B80EC,
                0x3F6284B7, 0x3DB3443C, 0x3DDCAABA, 0x3E9D628C, 0x3EE869F3, 0x50000000, 0x50000000,
            ],
            [
                0x3F26C524, 0x3F8312D6, 0x3ECDA605, 0x3E9F5E32, 0x3F8312D6, 0x1038C83E, 0x1038C83E,
            ],
        ),
        (
            [
                0x3F246605, 0x3E16E1E7, 0x3F108762, 0x3DF3712B, 0x3F12B67F, 0x3F2648EB, 0x3ECF1DD3,
                0x3F012F48, 0x3E800D3D, 0x3EC0C6A4, 0x3F41F11F, 0x3F58BC7F, 0x10000000, 0x10000000,
            ],
            [
                0x3F108762, 0x3F4F76FB, 0x3EDABE07, 0x3F265598, 0x3F4F76FB, 0x05DE81C8, 0x05DE81C8,
            ],
        ),
        (
            [
                0x3DF70C81, 0x3E27BAF3, 0x3F5197E3, 0x3F247DE2, 0x3EE80245, 0x3E972058, 0x3F51C318,
                0x3ECD0EAA, 0x3F2BC501, 0x3F7D5E64, 0x3EB7DEB8, 0x3F123909, 0x78000000, 0x78000000,
            ],
            [
                0x3F5197E3, 0x3F058619, 0x3E6A4018, 0x3F268C88, 0x3F058619, 0x3FEAE991, 0x3FEAE991,
            ],
        ),
        (
            [
                0x3F4240AF, 0x3F6CBEA1, 0x3EE99C36, 0x3ECD3554, 0x3DFF24F9, 0x3E03F697, 0x3F160475,
                0x3F02B79A, 0x3F3378B5, 0x3EAAF880, 0x3F496702, 0x3DFA6B7D, 0x70000000, 0x70000000,
            ],
            [
                0x3EE99C36, 0x3E195001, 0x3D7FBC5D, 0x3EE544BD, 0x3E195001, 0x16E210FB, 0x16E210FB,
            ],
        ),
    ];
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

    /// 47 random records with fixed Q31 multipliers (`0x10000000 .. 0x7FFFFFFF`): the model
    /// equals the oracle in 41; the 6 others are one LSB (4) or the saturated `q` (2), see
    /// `docs/research/smp-slew.md` section 6. All 6 records with the firmware multiplier
    /// `0x70000000` are exact.
    #[test]
    fn multiplier_set_against_the_oracle() {
        use super::goldens::TIE_CASES;
        let (mut exact, mut firmware) = (0, 0);
        for (input, want) in TIE_CASES.iter() {
            let mut r = record(input);
            let g = r.step();
            let hit = (g.left as u32, g.right as u32) == (want[5], want[6]);
            exact += usize::from(hit);
            if input[12] == 0x7000_0000 {
                assert!(hit, "firmware multiplier must be exact");
                firmware += 1;
            }
        }
        assert_eq!((exact, firmware), (41, 6));
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
