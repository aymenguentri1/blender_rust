//! Port of `source/blender/blenlib/intern/math_half.cc` (scalar `math::half_to_float`,
//! `math::float_to_half` and `math::float_to_half_make_finite`, the portable paths based on
//! https://gist.github.com/rygorous/2144712 and https://gist.github.com/rygorous/2156668).
//! Tests are the `half_to_float_scalar`, `float_to_half_scalar` and
//! `float_to_half_make_finite_scalar` cases of `tests/BLI_math_half_test.cc`.

/// `math::half_to_float()`: converts half precision (FP16 bits) to float (FP32),
/// including denormals, infinities and NaNs.
pub fn half_to_float(v: u16) -> f32 {
    const MAGIC: f32 = f32::from_bits(113 << 23);
    const SHIFTED_EXP: u32 = 0x7c00 << 13; /* exponent mask after shift */

    let mut o: u32 = ((v & 0x7fff) as u32) << 13; /* exponent/mantissa bits */
    let exp = SHIFTED_EXP & o; /* just the exponent */
    o = o.wrapping_add((127 - 15) << 23); /* exponent adjust */

    let mut f;
    /* handle exponent special cases */
    if exp == SHIFTED_EXP {
        /* Inf/NaN: extra exp adjust. */
        o = o.wrapping_add((128 - 16) << 23);
        f = f32::from_bits(o);
    } else if exp == 0 {
        /* Zero/Denormal: extra exp adjust, then renormalize. */
        o = o.wrapping_add(1 << 23);
        f = f32::from_bits(o) - MAGIC;
    } else {
        f = f32::from_bits(o);
    }

    /* sign bit */
    f = f32::from_bits(f.to_bits() | (((v & 0x8000) as u32) << 16));
    f
}

/// `math::float_to_half()`: converts float (FP32) to half precision (FP16 bits), rounding to
/// nearest even; infinities and overflow give FP16 infinity, NaN gives the quiet NaN 0x7e00.
pub fn float_to_half(v: f32) -> u16 {
    const F32_INFTY: u32 = 255 << 23;
    const F16_MAX: u32 = (127 + 16) << 23;
    const DENORM_MAGIC: u32 = ((127 - 15) + (23 - 10) + 1) << 23;
    const SIGN_MASK: u32 = 0x8000_0000;

    let mut u = v.to_bits();
    let sign = u & SIGN_MASK;
    u ^= sign;

    let mut o: u16;
    if u >= F16_MAX {
        /* result is Inf or NaN (all exponent bits set): NaN->qNaN and Inf->Inf */
        o = if u > F32_INFTY { 0x7e00 } else { 0x7c00 };
    } else if u < (113 << 23) {
        /* Resulting FP16 is subnormal or zero. A magic value aligns the 10 mantissa bits at the
         * bottom of the float; round-to-nearest-even addition then does the rounding. */
        let f = f32::from_bits(u) + f32::from_bits(DENORM_MAGIC);
        o = f.to_bits().wrapping_sub(DENORM_MAGIC) as u16;
    } else {
        let mant_odd = (u >> 13) & 1; /* resulting mantissa is odd */
        /* update exponent, rounding bias part 1 */
        u = u.wrapping_add((((15i32 - 127) as u32) << 23).wrapping_add(0xfff));
        /* rounding bias part 2 */
        u = u.wrapping_add(mant_odd);
        o = (u >> 13) as u16;
    }

    o |= (sign >> 16) as u16;
    o
}

/// `math::float_to_half_make_finite()`: like `float_to_half()`, but +/- infinity becomes
/// +/- the maximum value (65504) and NaN becomes zero (keeping the sign bit).
pub fn float_to_half_make_finite(v: f32) -> u16 {
    let mut h = float_to_half(v);
    /* Infinity or NaN? */
    if (h & 0x7c00) == 0x7c00 {
        if (h & 0x03ff) == 0 {
            /* +/- infinity: +/- max value. */
            h ^= 0x07ff;
        } else {
            /* +/- NaN: +/- zero. */
            h &= 0x8000;
        }
    }
    h
}


#[cfg(test)]
mod tests {
    use super::*;

    /// (value, `float_to_half`, `float_to_half_make_finite`)
    #[test]
    fn float_to_half_scalar() {
        let expected: [(f32, u16, u16); 31] = [
            (0.0, 0, 0),
            (f32::MIN_POSITIVE, 0, 0),
            (5.960_464_5e-8, 1, 1),
            (1.907_348_6e-6, 32, 32),
            (2.205_371_9e-6, 37, 37),
            (3.045_797_3e-5, 511, 511),
            (5.954_504e-5, 999, 999),
            (6.103_515_6e-5, 1024, 1024),
            (8.088_35e-5, 1357, 1357),
            (0.003_183_364_9, 6789, 6789),
            (0.1, 11878, 11878),
            (1.0, 15360, 15360),
            (1.999_023_4, 16383, 16383),
            (1.999_523_4, 16384, 16384),
            (2.0, 16384, 16384),
            (11.0, 18816, 18816),
            (65504.0, 31743, 31743),
            (65535.0, 31744, 31743), /* FP16 inf / too large */
            (1.0e6, 31744, 31743),
            (f32::INFINITY, 31744, 31743),
            (f32::MAX, 31744, 31743),
            (-0.0, 32768, 32768),
            (-5.960_464_5e-8, 32769, 32769),
            (-0.417_236_33, 46765, 46765),
            (-1.0, 48128, 48128),
            (-78.3125, 54501, 54501),
            (-123.5, 55224, 55224),
            (-65504.0, 64511, 64511),
            (-65536.0, 64512, 64511), /* FP16 -inf / too large negative */
            (-1.0e6, 64512, 64511),
            (f32::NEG_INFINITY, 64512, 64511),
        ];
        for (f, half, finite) in expected {
            assert_eq!(float_to_half(f), half, "float_to_half({f})");
            assert_eq!(float_to_half_make_finite(f), finite, "float_to_half_make_finite({f})");
        }
        assert_eq!(float_to_half(f32::NAN), 32256);
        /* NaN: result is zero. */
        assert_eq!(float_to_half_make_finite(f32::NAN), 0);
        assert_eq!(float_to_half_make_finite(f32::from_bits(0x7f80_0001)), 0);
    }

    #[test]
    fn half_to_float_scalar() {
        let expected: [(u16, f32); 19] = [
            (0, 0.0),
            (1, 5.960_464_5e-8),
            (32, 1.907_348_6e-6),
            (37, 2.205_371_9e-6),
            (511, 3.045_797_3e-5),
            (999, 5.954_504e-5),
            (1024, 6.103_515_6e-5),
            (1357, 8.088_35e-5),
            (6789, 0.003_183_364_9),
            (16383, 1.999_023_4),
            (16384, 2.0),
            (31743, 65504.0),
            (31744, f32::INFINITY),
            (32768, -0.0),
            (32769, -5.960_464_5e-8),
            (46765, -0.417_236_33),
            (54501, -78.3125),
            (64511, -65504.0),
            (64512, f32::NEG_INFINITY),
        ];
        for (h, f) in expected {
            let r = half_to_float(h);
            assert_eq!(r, f, "half_to_float({h})");
            assert_eq!(r.is_sign_negative(), f.is_sign_negative(), "sign of half_to_float({h})");
        }
        for h in [31746u16, 32767, 64513, 65535] {
            assert!(half_to_float(h).is_nan(), "half_to_float({h})");
        }
    }
}
