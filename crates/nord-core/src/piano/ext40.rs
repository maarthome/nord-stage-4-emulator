//! 40-bit extended-precision float of the SHARC data registers (8-bit exponent, 32-bit mantissa),
//! as the SMP audio interrupt runs it: `MODE1 = 0x1001CFC` leaves `RND32` clear (extended
//! precision) and sets `TRUNCATE` (round toward zero).
//!
//! VERIFIED: the model reproduces the SMP slew loop (`smp_slew.rs`) of the `sharc-ref` oracle bit
//! for bit in 1,000+ field comparisons only when intermediate results keep 32 mantissa bits and
//! every operation truncates; a store to memory drops the 8 low mantissa bits (`to_f32_bits`).
//! Normal numbers only: zero, denormals and non-finite inputs are flushed to zero (ASSUMED, the
//! slew fields are small positive floats and never hold them).
//!
//! Real-time safe: integer arithmetic only, no allocation.

/// `value = (-1)^neg * m * 2^exp` with `m` normalised to 32 bits (`bit 31` set) or zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ext {
    neg: bool,
    exp: i32,
    m: u64,
}

impl Ext {
    pub const ZERO: Ext = Ext {
        neg: false,
        exp: 0,
        m: 0,
    };

    /// Exact conversion of a normal `f32` (24-bit mantissa) into the 32-bit-mantissa format.
    pub fn from_bits(w: u32) -> Ext {
        let e = ((w >> 23) & 0xFF) as i32;
        if e == 0 || e == 0xFF {
            return Ext::ZERO;
        }
        let m24 = u64::from((w & 0x7F_FFFF) | 0x80_0000);
        Ext {
            neg: w >> 31 == 1,
            exp: e - 158,
            m: m24 << 8,
        }
    }

    /// Store to a 32-bit memory word: the 8 low mantissa bits are dropped (truncation).
    pub fn to_bits(self) -> u32 {
        if self.m == 0 {
            return 0;
        }
        let e = self.exp + 158;
        if e <= 0 {
            return 0;
        }
        if e >= 255 {
            return (u32::from(self.neg) << 31) | 0x7F7F_FFFF;
        }
        (u32::from(self.neg) << 31) | ((e as u32) << 23) | (((self.m >> 8) as u32) & 0x7F_FFFF)
    }

    pub fn is_neg(self) -> bool {
        self.neg && self.m != 0
    }

    fn abs(self) -> Ext {
        Ext { neg: false, ..self }
    }

    fn neg(self) -> Ext {
        Ext {
            neg: !self.neg,
            ..self
        }
    }

    /// Normalises a magnitude `mag * 2^exp` (any width up to 128 bits) to 32 bits, truncating.
    fn from_mag(neg: bool, mag: u128, exp: i32) -> Ext {
        if mag == 0 {
            return Ext::ZERO;
        }
        let bits = 128 - mag.leading_zeros() as i32;
        let (m, e) = if bits > 32 {
            let sh = bits - 32;
            ((mag >> sh) as u64, exp + sh)
        } else {
            ((mag << (32 - bits)) as u64, exp - (32 - bits))
        };
        Ext { neg, exp: e, m }
    }

    pub fn times(self, o: Ext) -> Ext {
        Ext::from_mag(
            self.neg != o.neg,
            u128::from(self.m) * u128::from(o.m),
            self.exp + o.exp,
        )
    }

    pub fn plus(self, o: Ext) -> Ext {
        if self.m == 0 {
            return o;
        }
        if o.m == 0 {
            return self;
        }
        // Align to the smaller exponent; a gap above 64 bits only matters as a sticky bit.
        let lo = self.exp.min(o.exp);
        let sh = |x: Ext| -> i128 {
            let s = (x.exp - lo).min(64) as u32;
            let v = (i128::from(x.m)) << s;
            if x.neg {
                -v
            } else {
                v
            }
        };
        let (a, b) = (sh(self), sh(o));
        let sum = a + b;
        Ext::from_mag(sum < 0, sum.unsigned_abs(), lo)
    }

    fn cmp_mag(self, o: Ext) -> std::cmp::Ordering {
        match (self.m == 0, o.m == 0) {
            (true, true) => return std::cmp::Ordering::Equal,
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        (self.exp, self.m).cmp(&(o.exp, o.m))
    }

    /// `Fn = MAX (Fx, Fy)`.
    pub fn max(self, o: Ext) -> Ext {
        if self.less(o) {
            o
        } else {
            self
        }
    }

    pub fn less(self, o: Ext) -> bool {
        match (self.is_neg(), o.is_neg()) {
            (true, false) => !(self.m == 0 && o.m == 0),
            (false, true) => false,
            (false, false) => self.cmp_mag(o) == std::cmp::Ordering::Less,
            (true, true) => self.cmp_mag(o) == std::cmp::Ordering::Greater,
        }
    }

    /// `Fn = CLIP Fx BY Fy`: limits `x` to `[-|y|, |y|]`.
    pub fn clip(self, limit: Ext) -> Ext {
        let hi = limit.abs();
        if hi.less(self) {
            hi
        } else if self.less(hi.neg()) {
            hi.neg()
        } else {
            self
        }
    }

    /// `Rn = FIX Fx BY 31`: round to nearest (ties up), saturating at the `i32` range.
    pub fn fix_q31(self) -> i32 {
        if self.m == 0 {
            return 0;
        }
        let sh = self.exp + 31;
        let mag: u128 = if sh >= 0 {
            if sh > 32 {
                u128::from(u32::MAX)
            } else {
                u128::from(self.m) << sh
            }
        } else if -sh > 40 {
            0
        } else {
            (u128::from(self.m) + (1u128 << (-sh - 1))) >> -sh
        };
        if self.neg {
            -(mag.min(1 << 31) as i64) as i32
        } else {
            mag.min(0x7FFF_FFFF) as i32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(x: f32) -> Ext {
        Ext::from_bits(x.to_bits())
    }

    #[test]
    fn f32_round_trips_exactly() {
        for x in [1.0f32, 0.979, 0.25, 12345.678, -0.3, 1e-20] {
            assert_eq!(e(x).to_bits(), x.to_bits());
        }
        assert_eq!(Ext::ZERO.to_bits(), 0);
    }

    #[test]
    fn product_keeps_32_mantissa_bits_and_stores_truncated() {
        // 0.979f32 * 0.979f32 needs 48 bits; the register keeps 32, a store keeps 24 (truncated).
        let p = e(0.979).times(e(0.979));
        let exact = f64::from(0.979f32) * f64::from(0.979f32);
        let stored = f64::from(f32::from_bits(p.to_bits()));
        assert!(stored <= exact && exact - stored < 2.0f64.powi(-24));
        // The register value is closer than the stored one.
        let kept = (p.m as f64) * 2.0f64.powi(p.exp);
        assert!((exact - kept).abs() < 2.0f64.powi(-31));
    }

    #[test]
    fn add_cancels_and_aligns() {
        assert_eq!(e(0.5).plus(e(0.25)).to_bits(), 0.75f32.to_bits());
        assert_eq!(e(0.5).plus(e(-0.5)).to_bits(), 0);
        assert_eq!(e(1.0).plus(e(-0.25)).to_bits(), 0.75f32.to_bits());
        // A tiny negative term truncates the sum toward zero (one unit below 1.0 in 32 bits).
        assert!(f32::from_bits(e(1.0).plus(e(-1e-30)).to_bits()) <= 1.0);
    }

    #[test]
    fn max_and_clip_follow_the_float_order() {
        assert_eq!(e(-0.5).max(Ext::ZERO).to_bits(), 0);
        assert_eq!(e(0.5).max(Ext::ZERO).to_bits(), 0.5f32.to_bits());
        assert_eq!(e(2.0).clip(e(0.75)).to_bits(), 0.75f32.to_bits());
        assert_eq!(e(-2.0).clip(e(-0.75)).to_bits(), (-0.75f32).to_bits());
        assert_eq!(e(0.25).clip(e(0.75)).to_bits(), 0.25f32.to_bits());
    }

    #[test]
    fn fix_q31_rounds_to_nearest_and_saturates() {
        assert_eq!(e(0.5).fix_q31(), 1 << 30);
        assert_eq!(e(1.0).fix_q31(), i32::MAX);
        assert_eq!(e(2.5).fix_q31(), i32::MAX);
        assert_eq!(e(-0.5).fix_q31(), -(1 << 30));
        assert_eq!(Ext::ZERO.fix_q31(), 0);
        // 3 * 2^-33 * 2^31 = 0.75 -> 1; 2^-33 * 2^31 = 0.25 -> 0.
        assert_eq!(e(3.0 * 2.0f32.powi(-33)).fix_q31(), 1);
        assert_eq!(e(2.0f32.powi(-33)).fix_q31(), 0);
    }
}
