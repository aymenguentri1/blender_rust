//! Port of `source/blender/blenlib/intern/math_solvers.cc` (`BLI_tridiagonal_solve`,
//! `BLI_tridiagonal_solve_cyclic`).
//! Tests are the `Tridiagonal1` / `Tridiagonal3` / `CyclicTridiagonal1` / `CyclicTridiagonal2` /
//! `CyclicTridiagonal3` cases of `tests/BLI_math_solvers_test.cc`.

/// `BLI_tridiagonal_solve()`: solves `a[i] * x[i-1] + b[i] * x[i] + c[i] * x[i+1] = d[i]` with the
/// Thomas algorithm, in double precision like the source. `a[0]` and `c[count - 1]` are ignored.
/// All slices must have the length `count = d.len()`. Unlike the C version, `r_x` cannot alias the
/// inputs. Returns false for `count < 1` or a non-finite result.
pub fn tridiagonal_solve(a: &[f32], b: &[f32], c: &[f32], d: &[f32], r_x: &mut [f32]) -> bool {
    let count = d.len();
    if count < 1 {
        return false;
    }
    debug_assert!(a.len() == count && b.len() == count && c.len() == count && r_x.len() == count);

    let mut c1 = vec![0.0f64; count];
    let mut d1 = vec![0.0f64; count];

    /* forward pass */

    let mut c_prev = c[0] as f64 / b[0] as f64;
    let mut d_prev = d[0] as f64 / b[0] as f64;
    c1[0] = c_prev;
    d1[0] = d_prev;

    for i in 1..count {
        let denum = b[i] as f64 - a[i] as f64 * c_prev;

        c_prev = c[i] as f64 / denum;
        c1[i] = c_prev;
        d_prev = (d[i] as f64 - a[i] as f64 * d_prev) / denum;
        d1[i] = d_prev;
    }

    /* back pass */

    let mut x_prev = d_prev;
    r_x[count - 1] = x_prev as f32;

    for i in (0..count - 1).rev() {
        x_prev = d1[i] - c1[i] * x_prev;
        r_x[i] = x_prev as f32;
    }

    x_prev.is_finite()
}

/// `BLI_tridiagonal_solve_cyclic()`: like `tridiagonal_solve()`, but the system is cyclic:
/// `a[0]` multiplies `x[count - 1]` and `c[count - 1]` multiplies `x[0]`. All slices must have the
/// length `count = d.len()`. Returns false for `count < 1` or a non-finite result.
pub fn tridiagonal_solve_cyclic(a: &[f32], b: &[f32], c: &[f32], d: &[f32], r_x: &mut [f32]) -> bool {
    let count = d.len();
    if count < 1 {
        return false;
    }
    debug_assert!(a.len() == count && b.len() == count && c.len() == count && r_x.len() == count);

    /* Degenerate case not handled correctly by the generic formula. */
    if count == 1 {
        r_x[0] = d[0] / (a[0] + b[0] + c[0]);

        return r_x[0].is_finite();
    }

    /* Degenerate case that works but can be simplified. */
    if count == 2 {
        let a2 = [0.0, a[1] + c[1]];
        let c2 = [a[0] + c[0], 0.0];

        return tridiagonal_solve(&a2, b, &c2, d, r_x);
    }

    /* If not really cyclic, fall back to the simple solver. */
    let a0 = a[0];
    let c_n = c[count - 1];

    if a0 == 0.0 && c_n == 0.0 {
        return tridiagonal_solve(a, b, c, d, r_x);
    }

    /* Prepare the non-cyclic system; relies on `tridiagonal_solve()` ignoring values. */
    let mut b2 = b.to_vec();
    b2[0] -= a0;
    b2[count - 1] -= c_n;

    let mut adjust = vec![0.0f32; count];
    adjust[0] = a0;
    adjust[count - 1] = c_n;

    /* solve for partial solution and adjustment vector (the C version solves `tmp` in place) */
    let rhs = adjust.clone();
    let success = tridiagonal_solve(a, &b2, c, &rhs, &mut adjust) && tridiagonal_solve(a, &b2, c, d, r_x);

    /* apply adjustment */
    if success {
        let coeff = (r_x[0] + r_x[count - 1]) / (1.0 + adjust[0] + adjust[count - 1]);

        for i in 0..count {
            r_x[i] -= coeff * adjust[i];
        }
    }

    success
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyclic_tridiagonal1() {
        let mut x = [0.0f32; 1];

        assert!(tridiagonal_solve_cyclic(&[1.0], &[2.0], &[1.0], &[4.0], &mut x));
        assert!(float_eq(x[0], 1.0));
    }

    #[test]
    fn cyclic_tridiagonal2() {
        let mut x = [0.0f32; 2];

        assert!(tridiagonal_solve_cyclic(&[1.0, 2.0], &[3.0, 4.0], &[5.0, 6.0], &[15.0, 16.0], &mut x));
        assert!(float_eq(x[0], 1.0));
        assert!(float_eq(x[1], 2.0));
    }

    #[test]
    fn cyclic_tridiagonal3() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        let c = [7.0, 8.0, 9.0];
        let d = [21.0, 36.0, 33.0];
        let mut x = [0.0f32; 3];

        assert!(tridiagonal_solve_cyclic(&a, &b, &c, &d, &mut x));
        assert!(float_eq(x[0], 1.0));
        assert!(float_eq(x[1], 2.0));
        assert!(float_eq(x[2], 3.0));
    }

    /// gtest `EXPECT_FLOAT_EQ`: equal within 4 ULPs.
    fn float_eq(a: f32, b: f32) -> bool {
        (a.to_bits() as i32 - b.to_bits() as i32).unsigned_abs() <= 4
    }

    #[test]
    fn tridiagonal1() {
        let a = [1.0]; /* ignored */
        let b = [2.0];
        let c = [1.0]; /* ignored */
        let d = [4.0];
        let mut x = [0.0f32; 1];

        assert!(tridiagonal_solve(&a, &b, &c, &d, &mut x));
        assert!(float_eq(x[0], 2.0));
    }

    #[test]
    fn tridiagonal3() {
        let a = [1.0, 2.0, 3.0]; /* 1 ignored */
        let b = [4.0, 5.0, 6.0];
        let c = [7.0, 8.0, 9.0]; /* 9 ignored */
        let d = [18.0, 36.0, 24.0];
        let mut x = [0.0f32; 3];

        assert!(tridiagonal_solve(&a, &b, &c, &d, &mut x));
        assert!(float_eq(x[0], 1.0));
        assert!(float_eq(x[1], 2.0));
        assert!(float_eq(x[2], 3.0));
    }

    #[test]
    fn empty_fails() {
        assert!(!tridiagonal_solve(&[], &[], &[], &[], &mut []));
    }
}
