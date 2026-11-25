//! Port of `source/blender/blenlib/intern/math_base_safe_inline.cc` (declared in
//! `BLI_math_base_safe.hh`): "safe" math where the output is not NaN if the input is not NaN.
//! Tests are the `SafePowf` / `SafeModf` / `SafeLogf` cases of `tests/BLI_math_base_safe_test.cc`;
//! the other functions are checked against the behavior of their source.

/// `safe_divide()`: `a / b`, or 0 when `b` is 0.
pub fn safe_divide(a: f32, b: f32) -> f32 {
    if b != 0.0 { a / b } else { 0.0 }
}

/// `safe_modf()`: `fmodf(a, b)`, or 0 when `b` is 0.
pub fn safe_modf(a: f32, b: f32) -> f32 {
    if b != 0.0 { a % b } else { 0.0 }
}

/// `safe_logf()`: logarithm of `a` in `base`; 0 when `a` or `base` is not positive.
pub fn safe_logf(a: f32, base: f32) -> f32 {
    if a <= 0.0 || base <= 0.0 {
        return 0.0;
    }
    safe_divide(a.ln(), base.ln())
}

/// `safe_sqrtf()`: square root of `max(a, 0)`.
pub fn safe_sqrtf(a: f32) -> f32 {
    a.max(0.0).sqrt()
}

/// `safe_inverse_sqrtf()`: `1 / sqrt(a)` for positive `a`, else 0.
pub fn safe_inverse_sqrtf(a: f32) -> f32 {
    if a > 0.0 { 1.0 / a.sqrt() } else { 0.0 }
}

/// `CLAMP(a, -1, 1)` of the source (NaN passes through).
fn clamp_unit(a: f32) -> f32 {
    if a < -1.0 {
        -1.0
    } else if a > 1.0 {
        1.0
    } else {
        a
    }
}

/// `safe_asinf()`: `asinf()` of `a` clamped to `[-1, 1]`.
pub fn safe_asinf(a: f32) -> f32 {
    clamp_unit(a).asin()
}

/// `safe_acosf()`: `acosf()` of `a` clamped to `[-1, 1]`.
pub fn safe_acosf(a: f32) -> f32 {
    clamp_unit(a).acos()
}

/// `safe_powf()`: `powf()`, but 0 for a negative base with a non-integer exponent.
pub fn safe_powf(base: f32, exponent: f32) -> f32 {
    if base < 0.0 && exponent != (exponent as i32) as f32 {
        return 0.0;
    }
    base.powf(exponent)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// gtest `EXPECT_FLOAT_EQ`: equal within 4 ULPs.
    fn float_eq(a: f32, b: f32) -> bool {
        (a.to_bits() as i32 - b.to_bits() as i32).unsigned_abs() <= 4
    }

    #[test]
    fn safe_powf_cases() {
        let expected: [(f32, f32, f32); 8] = [
            (4.0, 3.0, 64.0),
            (3.2, 5.6, 674.2793796),
            (4.0, -2.0, 0.0625),
            (6.0, -3.2, 0.003235311),
            (-4.0, 6.0, 4096.0),
            (-3.0, 5.5, 0.0),
            (-2.5, -4.0, 0.0256),
            (-3.7, -4.5, 0.0),
        ];
        for (base, exponent, r) in expected {
            assert!(float_eq(safe_powf(base, exponent), r), "safe_powf({base}, {exponent})");
        }
    }

    #[test]
    fn safe_modf_cases() {
        let expected: [(f32, f32, f32); 6] = [
            (3.4, 2.2, 1.2),
            (3.4, -2.2, 1.2),
            (-3.4, -2.2, -1.2),
            (-3.4, 0.0, 0.0),
            (0.0, 3.0, 0.0),
            (55.0, 10.0, 5.0),
        ];
        for (a, b, r) in expected {
            assert!(float_eq(safe_modf(a, b), r), "safe_modf({a}, {b})");
        }
    }

    #[test]
    fn safe_logf_cases() {
        let expected: [(f32, f32, f32); 6] = [
            (3.3, 2.5, 1.302995247),
            (0.0, 3.0, 0.0),
            (3.0, 0.0, 0.0),
            (-2.0, 4.3, 0.0),
            (2.0, -4.3, 0.0),
            (-2.0, -4.3, 0.0),
        ];
        for (a, base, r) in expected {
            assert!(float_eq(safe_logf(a, base), r), "safe_logf({a}, {base})");
        }
    }

    #[test]
    fn safe_divide_cases() {
        assert_eq!(safe_divide(1.0, 4.0), 0.25);
        assert_eq!(safe_divide(1.0, 0.0), 0.0);
    }

    #[test]
    fn safe_sqrt_cases() {
        assert_eq!(safe_sqrtf(4.0), 2.0);
        assert_eq!(safe_sqrtf(-4.0), 0.0);
        assert_eq!(safe_inverse_sqrtf(4.0), 0.5);
        assert_eq!(safe_inverse_sqrtf(0.0), 0.0);
        assert_eq!(safe_inverse_sqrtf(-4.0), 0.0);
    }

    #[test]
    fn safe_asin_acos_cases() {
        use std::f32::consts::{FRAC_PI_2, PI};
        assert!(float_eq(safe_asinf(2.0), FRAC_PI_2));
        assert!(float_eq(safe_asinf(-2.0), -FRAC_PI_2));
        assert!(float_eq(safe_asinf(0.5), 0.5235988));
        assert_eq!(safe_acosf(2.0), 0.0);
        assert!(float_eq(safe_acosf(-2.0), PI));
        assert!(safe_acosf(f32::NAN).is_nan());
    }
}
