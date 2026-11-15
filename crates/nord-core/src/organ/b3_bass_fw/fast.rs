//! 64 bit fast paths of the 40 bit float add and multiply (normal operands).
//!
//! The general routines in `f40.rs` work on 128 bit integers so that every class (zero, infinity,
//! NaN) and every alignment is exact. For two normal operands the same result needs only 64 bits:
//! the 32 bit significands leave 31 guard bits below the rounding position, and an alignment shift
//! of 63 or more leaves a pure sticky bit. VERIFIED by `fast_paths_equal_the_general_routines`
//! (random operands, cancellation and every exponent distance against the 128 bit routines).

const MAG: u64 = 0x7F_FFFF_FFFF;
const FRAC: u64 = 0x7FFF_FFFF;

/// `(biased exponent, 32 bit significand)` of a normal operand, `None` for zero / inf / NaN.
#[inline(always)]
fn normal(v: u64) -> Option<(i32, u64)> {
    let e = ((v >> 31) & 0xFF) as i32;
    (e != 0 && e != 255).then_some((e, (1 << 31) | (v & FRAC)))
}

/// Rounds `m * 2^e2` (`m != 0`) to a 32 bit significand, nearest even, and packs the 40 bit float.
#[inline(always)]
fn round_pack(neg: bool, e2: i32, m: u64, sticky: bool) -> u64 {
    let sign = u64::from(neg) << 39;
    let msb = 63 - m.leading_zeros() as i32;
    let mut exp = e2 + msb;
    let shift = msb - 31;
    let mut q = if shift > 0 {
        let s = shift as u32;
        let rem = m & ((1u64 << s) - 1);
        let half = 1u64 << (s - 1);
        let q = m >> s;
        if rem > half || (rem == half && (sticky || q & 1 == 1)) {
            q + 1
        } else {
            q
        }
    } else {
        m << (-shift) as u32
    };
    if q == 1 << 32 {
        q >>= 1;
        exp += 1;
    }
    if exp > 127 {
        return sign | (0xFF << 31);
    }
    if exp < -126 {
        return sign;
    }
    sign | (((exp + 127) as u64) << 31) | (q & FRAC)
}

/// `Fx + Fy` (`sub` = `Fx - Fy`) for two normal operands, else `None`.
#[inline(always)]
pub(super) fn add(a: u64, b: u64, sub: bool) -> Option<u64> {
    let (ea, ma) = normal(a)?;
    let (eb, mb) = normal(b)?;
    let (neg_a, neg_b) = (a >> 39 == 1, (b >> 39 == 1) != sub);
    // the larger magnitude first (the exponent and fraction fields order like the magnitude)
    let (xn, xe, xm, yn, ye, ym) = if a & MAG >= b & MAG {
        (neg_a, ea, ma, neg_b, eb, mb)
    } else {
        (neg_b, eb, mb, neg_a, ea, ma)
    };
    let d = (xe - ye) as u32;
    let (big, full) = (xm << 31, ym << 31);
    let (small, sticky) = if d >= 63 {
        (0, true)
    } else {
        (full >> d, full & ((1u64 << d) - 1) != 0)
    };
    let m = if xn == yn {
        big + small
    } else {
        big - small - u64::from(sticky)
    };
    if m == 0 && !sticky {
        return Some(0);
    }
    Some(round_pack(xn, xe - 127 - 62, m, sticky))
}

/// `Fx * Fy` for two normal operands, else `None`.
#[inline(always)]
pub(super) fn mul(a: u64, b: u64) -> Option<u64> {
    let (ea, ma) = normal(a)?;
    let (eb, mb) = normal(b)?;
    Some(round_pack(
        (a ^ b) >> 39 == 1,
        ea + eb - 254 - 62,
        ma * mb,
        false,
    ))
}

#[cfg(test)]
mod tests {
    use super::super::f40;

    /// xorshift64 (deterministic test data).
    fn next(s: &mut u64) -> u64 {
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s
    }

    fn operand(s: &mut u64, lo: u64, span: u64) -> u64 {
        let r = next(s);
        let e = lo + (r >> 40) % span;
        ((r & 1) << 39) | (e << 31) | ((r >> 8) & 0x7FFF_FFFF)
    }

    #[test]
    fn fast_paths_equal_the_general_routines() {
        let mut s = 0x1234_5678_9ABC_DEF1u64;
        for i in 0..400_000 {
            // exponents spread over the whole range, and close together (cancellation)
            let (lo, span) = if i % 2 == 0 { (1, 254) } else { (120, 12) };
            let (a, mut b) = (operand(&mut s, lo, span), operand(&mut s, lo, span));
            if i % 5 == 0 {
                b = (a ^ (1 << 39)) ^ (next(&mut s) & 0x7); // nearly equal magnitude, other sign
            }
            for sub in [false, true] {
                assert_eq!(
                    super::add(a, b, sub),
                    Some(f40::add_general(a, b, sub)),
                    "add {a:#x} {b:#x} {sub}"
                );
            }
            assert_eq!(
                super::mul(a, b),
                Some(f40::mul_general(a, b)),
                "mul {a:#x} {b:#x}"
            );
        }
    }

    #[test]
    fn special_classes_fall_back() {
        let one = 0x7Fu64 << 31;
        assert_eq!(super::add(0, one, false), None);
        assert_eq!(super::mul(one, 0xFF << 31), None);
        assert_eq!(super::add(one, one, true), Some(0));
    }
}
