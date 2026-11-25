//! Port of `source/blender/blenlib/intern/math_base_inline.cc`
//! (`is_power_of_2_i`, `log2_floor_u`, `log2_ceil_u`, `floored_fmod`, `divide_ceil_u`,
//! `ceil_to_multiple_u`) and
//! `BLI_math_base.hh` (`math::mod_periodic`, `math::step`, `math::midpoint`, `math::interpolate`).
//! Tests are the `Log2FloorU` / `Log2CeilU` / `FlooredFMod` / `ModPeriodic` / `Step*` / `Midpoint` /
//! `InterpolateInt` cases of `tests/BLI_math_base_test.cc`. The other functions of
//! `math_base_inline.cc` here (`fractf`, `wrapf`, `pingpongf`, `smoothstep`, `smoothminf`,
//! `power_of_2_*`, `divide_round_i`, `divide_floor_i`, `round_to_even`, `round_fl_to_int`, `interpf`,
//! `ratiof`, `mod_i`, `pow*f`, `sqrt3f`, `sqrtf_signed`) have no test in the archive and are
//! checked against the behavior of their source.

/// `is_power_of_2_i()`: `(n & (n - 1)) == 0`, so 0 counts as a power of two.
/// The subtraction wraps (Blender relies on two's complement for `INT_MIN`).
pub fn is_power_of_2_i(n: i32) -> bool {
    (n & n.wrapping_sub(1)) == 0
}

/// `log2_floor_u()`: 0 for `x <= 1`, else `1 + log2_floor_u(x >> 1)`.
pub fn log2_floor_u(mut x: u32) -> u32 {
    let mut result = 0;
    while x > 1 {
        x >>= 1;
        result += 1;
    }
    result
}

/// `log2_ceil_u()`: `log2_floor_u(x)` for powers of two (and 0), else one more.
pub fn log2_ceil_u(x: u32) -> u32 {
    if is_power_of_2_i(x as i32) {
        log2_floor_u(x)
    } else {
        log2_floor_u(x) + 1
    }
}

/// `floored_fmod()`: `f - n * floorf(f / n)`, like Python `%` / GLSL `mod()`.
/// Result is in `[0, n)` for positive `n`.
pub fn floored_fmod(f: f32, n: f32) -> f32 {
    f - n * (f / n).floor()
}


/// `math::mod_periodic()` for floats: `a - b * floor(a / b)`.
/// Repeats the saw-tooth pattern on negative numbers: `mod_periodic(-3, 4) = 1`.
pub fn mod_periodic_f32(a: f32, b: f32) -> f32 {
    debug_assert!(b != 0.0);
    a - (b * (a / b).floor())
}

/// `math::mod_periodic()` for integers: `((a % b) + b) % b`. Undefined for negative `b`.
pub fn mod_periodic_i32(a: i32, b: i32) -> i32 {
    debug_assert!(b != 0);
    debug_assert!(i32::MAX - a.abs() >= b);
    ((a % b) + b) % b
}

/// `math::mod_periodic<int64_t>()`.
pub fn mod_periodic_i64(a: i64, b: i64) -> i64 {
    debug_assert!(b != 0);
    debug_assert!(i64::MAX - a.abs() >= b);
    ((a % b) + b) % b
}


/// `math::step()`: 0 if `value < edge`, else 1.
pub fn step_f32(edge: f32, value: f32) -> f32 {
    if value < edge { 0.0 } else { 1.0 }
}

/// `math::midpoint()` for floats: `(a + b) * 0.5`.
pub fn midpoint_f32(a: f32, b: f32) -> f32 {
    (a + b) * 0.5
}

/// `math::interpolate()` for floats: `a * (1 - t) + b * t`.
pub fn interpolate_f32(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

/// `math::interpolate()` for an integer type with a float factor: computed in float, then
/// rounded with `std::round` (halfway cases away from zero).
pub fn interpolate_i32(a: i32, b: i32, t: f32) -> i32 {
    (a as f32 * (1.0 - t) + b as f32 * t).round() as i32
}


/// `divide_ceil_u()`: `(a + b - 1) / b`, wrapping like C `unsigned int`. `b` must be positive.
pub fn divide_ceil_u(a: u32, b: u32) -> u32 {
    a.wrapping_add(b).wrapping_sub(1) / b
}

/// `ceil_to_multiple_u()`: `divide_ceil_u(a, b) * b`.
pub fn ceil_to_multiple_u(a: u32, b: u32) -> u32 {
    divide_ceil_u(a, b).wrapping_mul(b)
}


/// `fractf()`: `a - floorf(a)`.
pub fn fractf(a: f32) -> f32 {
    a - a.floor()
}

/// `wrapf()` (adapted from `godot-engine` `math_funcs.h`): wraps `value` into `[min, max)`;
/// `min` when the range is empty.
pub fn wrapf(value: f32, max: f32, min: f32) -> f32 {
    let range = max - min;
    if range != 0.0 { value - (range * ((value - min) / range).floor()) } else { min }
}

/// `pingpongf()`: bounces `value` between 0 and `scale`.
pub fn pingpongf(value: f32, scale: f32) -> f32 {
    if scale == 0.0 {
        return 0.0;
    }
    (fractf((value - scale) / (scale * 2.0)) * scale * 2.0 - scale).abs()
}

/// `min_ff()`.
fn min_ff(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// `max_ff()`.
fn max_ff(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// `smoothminf()`: polynomial smooth minimum, see https://www.iquilezles.org/www/articles/smin/smin.htm.
pub fn smoothminf(a: f32, b: f32, c: f32) -> f32 {
    if c != 0.0 {
        let h = max_ff(c - (a - b).abs(), 0.0) / c;
        return min_ff(a, b) - h * h * h * c * (1.0 / 6.0);
    }
    min_ff(a, b)
}

/// `smoothstep()`.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if x < edge0 {
        0.0
    } else if x >= edge1 {
        1.0
    } else {
        let t = (x - edge0) / (edge1 - edge0);
        (3.0 - 2.0 * t) * (t * t)
    }
}

/// `power_of_2_max_i()`: smallest power of two not below `n` (wrapping like C `int`).
pub fn power_of_2_max_i(mut n: i32) -> i32 {
    if is_power_of_2_i(n) {
        return n;
    }
    loop {
        n &= n.wrapping_sub(1);
        if is_power_of_2_i(n) {
            break;
        }
    }
    n.wrapping_mul(2)
}

/// `power_of_2_min_i()`: largest power of two not above `n`.
pub fn power_of_2_min_i(mut n: i32) -> i32 {
    while !is_power_of_2_i(n) {
        n &= n.wrapping_sub(1);
    }
    n
}

/// `power_of_2_max_u()`: smallest power of two not below `x` (0 maps to 0).
pub fn power_of_2_max_u(mut x: u32) -> u32 {
    x = x.wrapping_sub(1);
    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;
    x.wrapping_add(1)
}

/// `divide_round_i()`: `(2 * a + b) / (2 * b)`.
pub fn divide_round_i(a: i32, b: i32) -> i32 {
    (2 * a + b) / (2 * b)
}

/// `divide_floor_i()`: integer division that floors a negative result (like Python `//`).
pub fn divide_floor_i(a: i32, b: i32) -> i32 {
    let d = a / b;
    let r = a % b;
    if r != 0 { d - ((a < 0) ^ (b < 0)) as i32 } else { d }
}

/// `round_to_even()`: `roundf(f * 0.5) * 2`.
pub fn round_to_even(f: f32) -> f32 {
    (f * 0.5).round() * 2.0
}

/// `round_fl_to_int()`: `(int)floorf(a + 0.5)`.
pub fn round_fl_to_int(a: f32) -> i32 {
    (a + 0.5).floor() as i32
}

/// `interpf()`: `fac * target + (1 - fac) * origin`.
pub fn interpf(target: f32, origin: f32, fac: f32) -> f32 {
    (fac * target) + (1.0 - fac) * origin
}

/// `ratiof()`: position of `pos` between `min` and `max`; 0 for an empty range.
pub fn ratiof(min: f32, max: f32, pos: f32) -> f32 {
    let range = max - min;
    if range == 0.0 { 0.0 } else { (pos - min) / range }
}

/// `mod_i()`: `(i % n + n) % n`.
pub fn mod_i(i: i32, n: i32) -> i32 {
    (i % n + n) % n
}

/// `pow2f()`.
pub fn pow2f(x: f32) -> f32 {
    x * x
}

/// `pow3f()`.
pub fn pow3f(x: f32) -> f32 {
    pow2f(x) * x
}

/// `pow4f()`.
pub fn pow4f(x: f32) -> f32 {
    pow2f(pow2f(x))
}

/// `pow5f()`.
pub fn pow5f(x: f32) -> f32 {
    pow4f(x) * x
}

/// `pow7f()`.
pub fn pow7f(x: f32) -> f32 {
    pow2f(pow3f(x)) * x
}

/// `sqrt3f()`: real cube root, computed in double precision like the source.
pub fn sqrt3f(f: f32) -> f32 {
    if f == 0.0 {
        0.0
    } else if f < 0.0 {
        -(((-f) as f64).ln() / 3.0).exp() as f32
    } else {
        ((f as f64).ln() / 3.0).exp() as f32
    }
}

/// `sqrtf_signed()`: square root keeping the sign of the input.
pub fn sqrtf_signed(f: f32) -> f32 {
    if f >= 0.0 { f.sqrt() } else { -(-f).sqrt() }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fract_wrap_pingpong_cases() {
        assert_eq!(fractf(1.25), 0.25);
        assert_eq!(fractf(-0.25), 0.75);
        assert_eq!(wrapf(4.0, 3.0, 0.0), 1.0);
        assert_eq!(wrapf(-1.0, 3.0, 0.0), 2.0);
        assert_eq!(wrapf(5.0, 2.0, 2.0), 2.0);
        for (value, r) in [(0.0f32, 0.0f32), (0.5, 0.5), (1.0, 1.0), (1.5, 0.5), (2.0, 0.0)] {
            assert_eq!(pingpongf(value, 1.0), r, "pingpongf({value}, 1)");
        }
        assert_eq!(pingpongf(3.0, 0.0), 0.0);
    }

    #[test]
    fn smoothstep_smoothmin_cases() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 1.0), 1.0);
        assert_eq!(smoothstep(0.0, 1.0, 0.5), 0.5);
        assert_eq!(smoothstep(0.0, 1.0, 0.25), 0.15625);
        assert_eq!(smoothminf(1.0, 2.0, 0.0), 1.0);
        assert!(float_eq(smoothminf(1.0, 1.0, 1.0), 5.0 / 6.0));
    }

    #[test]
    fn power_of_2_cases() {
        for (n, max, min) in [(1, 1, 1), (3, 4, 2), (5, 8, 4), (1000, 1024, 512), (1024, 1024, 1024)] {
            assert_eq!(power_of_2_max_i(n), max, "power_of_2_max_i({n})");
            assert_eq!(power_of_2_min_i(n), min, "power_of_2_min_i({n})");
        }
        assert_eq!(power_of_2_max_i(0), 0);
        for (x, r) in [(0u32, 0u32), (1, 1), (3, 4), (5, 8), (1025, 2048), (1 << 31, 1 << 31)] {
            assert_eq!(power_of_2_max_u(x), r, "power_of_2_max_u({x})");
        }
    }

    #[test]
    fn divide_round_floor_cases() {
        for (a, b, r) in [(7, 2, 4), (5, 2, 3), (-5, 2, -2), (1, 3, 0), (2, 3, 1)] {
            assert_eq!(divide_round_i(a, b), r, "divide_round_i({a}, {b})");
        }
        for (a, b, r) in [(7, 2, 3), (-7, 2, -4), (7, -2, -4), (-7, -2, 3), (6, 3, 2), (-6, 3, -2)] {
            assert_eq!(divide_floor_i(a, b), r, "divide_floor_i({a}, {b})");
        }
    }

    #[test]
    fn rounding_cases() {
        for (f, r) in [(2.0f32, 2.0f32), (3.0, 4.0), (1.0, 2.0), (0.9, 0.0), (-1.0, -2.0), (4.0, 4.0), (5.0, 6.0)] {
            assert_eq!(round_to_even(f), r, "round_to_even({f})");
        }
        for (f, r) in [(0.5f32, 1), (1.5, 2), (-0.5, 0), (-1.5, -1), (2.4, 2)] {
            assert_eq!(round_fl_to_int(f), r, "round_fl_to_int({f})");
        }
    }

    #[test]
    fn interpf_ratiof_mod_i_cases() {
        assert_eq!(interpf(10.0, 0.0, 0.25), 2.5);
        assert_eq!(interpf(10.0, 20.0, 0.5), 15.0);
        assert_eq!(ratiof(0.0, 10.0, 2.5), 0.25);
        assert_eq!(ratiof(4.0, 4.0, 7.0), 0.0);
        assert_eq!(mod_i(-1, 10), 9);
        assert_eq!(mod_i(11, 10), 1);
    }

    #[test]
    fn pow_and_root_cases() {
        assert_eq!(pow2f(3.0), 9.0);
        assert_eq!(pow3f(2.0), 8.0);
        assert_eq!(pow4f(2.0), 16.0);
        assert_eq!(pow5f(2.0), 32.0);
        assert_eq!(pow7f(2.0), 128.0);
        assert!(float_eq(sqrt3f(27.0), 3.0));
        assert!(float_eq(sqrt3f(-8.0), -2.0));
        assert_eq!(sqrt3f(0.0), 0.0);
        assert_eq!(sqrtf_signed(9.0), 3.0);
        assert_eq!(sqrtf_signed(-9.0), -3.0);
    }

    /// Per-component cases of `math_vector.DivideCeil` and `math_vector.CeilToMultiple`
    /// (`a = (21, 16, 0)`, `b = (8, 16, 15)`).
    #[test]
    fn divide_ceil_u_cases() {
        assert_eq!(divide_ceil_u(21, 8), 3);
        assert_eq!(divide_ceil_u(16, 16), 1);
        assert_eq!(divide_ceil_u(0, 15), 0);
        assert_eq!(ceil_to_multiple_u(21, 8), 24);
        assert_eq!(ceil_to_multiple_u(16, 16), 16);
        assert_eq!(ceil_to_multiple_u(0, 15), 0);
    }

    #[test]
    fn step_cases() {
        assert_eq!(step_f32(0.5, 0.3), 0.0);
        assert_eq!(step_f32(0.5, 0.6), 1.0);
        assert_eq!(step_f32(0.5, 0.5), 1.0);
    }

    #[test]
    fn midpoint_case() {
        assert!((midpoint_f32(100.0, 200.0) - 150.0).abs() <= 1e-4);
    }

    #[test]
    fn interpolate_cases() {
        assert_eq!(interpolate_i32(100, 200, 0.4), 140);
        assert!((interpolate_f32(100.0, 200.0, 0.4) - 140.0).abs() <= 1e-4);
    }

    #[test]
    fn mod_periodic_f32_cases() {
        let expected: [(f32, f32, f32); 9] = [
            (3.27, 1.57, 0.12999988),
            (327.0, 47.0, 45.0),
            (-0.1, 1.0, 0.9),
            (-0.9, 1.0, 0.1),
            (-100.1, 1.0, 0.90000153),
            (-0.1, 12345.0, 12344.9),
            (12345.1, 12345.0, 0.099609375),
            (12344.999, 12345.0, 12344.999),
            (12345.0, 12345.0, 0.0),
        ];
        for (a, b, r) in expected {
            assert!(float_eq(mod_periodic_f32(a, b), r), "mod_periodic({a}, {b})");
        }
    }

    #[test]
    fn mod_periodic_int_cases() {
        let half = i32::MAX / 2;
        assert_eq!(mod_periodic_i32(1, 10), 1);
        assert_eq!(mod_periodic_i32(11, 10), 1);
        assert_eq!(mod_periodic_i32(-1, 10), 9);
        assert_eq!(mod_periodic_i32(-11, 10), 9);
        assert_eq!(mod_periodic_i32(1, 1), 0);
        assert_eq!(mod_periodic_i32(0, 99999), 0);
        assert_eq!(mod_periodic_i32(99999, 99999), 0);
        assert_eq!(mod_periodic_i32(half, half), 0);
        assert_eq!(mod_periodic_i32(i32::MIN / 2, half), half - 1);

        assert_eq!(mod_periodic_i64(1, 10), 1);
        assert_eq!(mod_periodic_i64(11, 10), 1);
        assert_eq!(mod_periodic_i64(-1, 10), 9);
        assert_eq!(mod_periodic_i64(-11, 10), 9);
        assert_eq!(mod_periodic_i64(1, 1), 0);
        assert_eq!(mod_periodic_i64(0, 99999), 0);
        assert_eq!(mod_periodic_i64(99999, 99999), 0);
    }

    /// gtest `EXPECT_FLOAT_EQ`: equal within 4 ULPs.
    fn float_eq(a: f32, b: f32) -> bool {
        (a.to_bits() as i32 - b.to_bits() as i32).unsigned_abs() <= 4
    }

    #[test]
    fn floored_fmod_cases() {
        let expected: [(f32, f32, f32); 9] = [
            (3.27, 1.57, 0.12999988),
            (327.0, 47.0, 45.0),
            (-0.1, 1.0, 0.9),
            (-0.9, 1.0, 0.1),
            (-100.1, 1.0, 0.90000153),
            (-0.1, 12345.0, 12344.9),
            (12345.1, 12345.0, 0.099609375),
            (12344.999, 12345.0, 12344.999),
            (12345.0, 12345.0, 0.0),
        ];
        for (f, n, r) in expected {
            assert!(float_eq(floored_fmod(f, n), r), "floored_fmod({f}, {n})");
        }
    }

    #[test]
    fn log2_floor_u_cases() {
        let expected = [(0, 0), (1, 0), (2, 1), (3, 1), (4, 2), (5, 2), (6, 2), (7, 2), (8, 3), (9, 3), (123456, 16)];
        for (x, r) in expected {
            assert_eq!(log2_floor_u(x), r, "log2_floor_u({x})");
        }
    }

    #[test]
    fn log2_ceil_u_cases() {
        let expected = [(0, 0), (1, 0), (2, 1), (3, 2), (4, 2), (5, 3), (6, 3), (7, 3), (8, 3), (9, 4), (123456, 17)];
        for (x, r) in expected {
            assert_eq!(log2_ceil_u(x), r, "log2_ceil_u({x})");
        }
    }

    #[test]
    fn is_power_of_2_i_cases() {
        assert!(is_power_of_2_i(0));
        assert!(is_power_of_2_i(1));
        assert!(is_power_of_2_i(64));
        assert!(!is_power_of_2_i(6));
        assert!(is_power_of_2_i(i32::MIN));
    }
}
