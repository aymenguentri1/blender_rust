//! Regularized Kelvinlets (`blenkernel/intern/kelvinlet.cc`, Pixar Technical Memo #17-03): the falloff of the
//! Elastic Deform brush. `a`, `b`, `c` are the elasticity constants of `BKE_kelvinlet_init_params()`.

use crate::mesh::{add, cross, scale, sub, V3};

/// `KELVINLET_MAX_ITERATIONS`.
const ITER: usize = 3;

/// `KelvinletParams`.
#[derive(Clone, Copy, Debug)]
pub struct KelvinletParams {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub f: f32,
    pub radius_scaled: [f32; ITER],
}

fn len(v: V3) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

impl KelvinletParams {
    /// `BKE_kelvinlet_init_params()`.
    pub fn new(radius: f32, force: f32, shear_modulus: f32, poisson_ratio: f32) -> Self {
        let a = 1.0 / (4.0 * std::f32::consts::PI * shear_modulus);
        let b = a / (4.0 * (1.0 - poisson_ratio));
        let c = 2.0 * (3.0 * a - 2.0 * b);
        let r0 = radius;
        let r1 = r0 * 2.0;
        let r2 = r1 * 2.0;
        Self { a, b, c, f: force, radius_scaled: [r0, r1, r2] }
    }

    /// `init_kelvinlet_grab()`: formula (6) for the first `n` scales.
    fn grab_kernel(&self, radius: f32, n: usize) -> [f32; ITER] {
        let (a, b) = (self.a, self.b);
        let mut k = [0.0; ITER];
        for i in 0..n {
            let rs = self.radius_scaled[i];
            let re = (radius * radius + rs * rs).sqrt();
            k[i] = (a - b) / re + (b * radius * radius) / (re * re * re) + (a * rs * rs) / (2.0 * re * re * re);
        }
        k
    }

    /// `BKE_kelvinlet_grab()`.
    pub fn grab(&self, co: V3, location: V3, delta: V3) -> V3 {
        let k = self.grab_kernel(len(sub(location, co)), 1);
        scale(delta, k[0] * self.c)
    }

    /// `BKE_kelvinlet_grab_biscale()`.
    pub fn grab_biscale(&self, co: V3, location: V3, delta: V3) -> V3 {
        let k = self.grab_kernel(len(sub(location, co)), 2);
        let rs = self.radius_scaled;
        let u = k[0] - k[1];
        scale(delta, u * self.c / (1.0 / rs[0] - 1.0 / rs[1]))
    }

    /// `BKE_kelvinlet_grab_triscale()`.
    pub fn grab_triscale(&self, co: V3, location: V3, delta: V3) -> V3 {
        let k = self.grab_kernel(len(sub(location, co)), 3);
        let rs = self.radius_scaled;
        let sq = |x: f32| x * x;
        let w = [
            1.0,
            -((sq(rs[2]) - sq(rs[0])) / (sq(rs[2]) - sq(rs[1]))),
            (sq(rs[1]) - sq(rs[0])) / (sq(rs[2]) - sq(rs[1])),
        ];
        let u = w[0] * k[0] + w[1] * k[1] + w[2] * k[2];
        let fade = u * self.c / (w[0] / rs[0] + w[1] / rs[1] + w[2] / rs[2]);
        scale(delta, fade)
    }

    /// `kelvinlet_scale()`: formula (16).
    fn scale_at(&self, co: V3, location: V3) -> V3 {
        let rv = sub(co, location);
        let r = len(rv);
        let rs0 = self.radius_scaled[0];
        let re = (r * r + rs0 * rs0).sqrt();
        let u = (2.0 * self.b - self.a) * (1.0 / (re * re * re)) + (3.0 * rs0 * rs0) / (2.0 * re.powi(5));
        scale(rv, u * self.c * self.f)
    }

    /// `kelvinlet_twist()`: formula (15).
    fn twist_at(&self, co: V3, location: V3, normal: V3) -> V3 {
        let rv = sub(co, location);
        let r = len(rv);
        let rs0 = self.radius_scaled[0];
        let re = (r * r + rs0 * rs0).sqrt();
        let u = -self.a * (1.0 / (re * re * re)) + (3.0 * rs0 * rs0) / (2.0 * re.powi(5));
        scale(cross(normal, rv), u * self.c * self.f)
    }

    /// `sculpt_kelvinet_integrate()`: a Runge-Kutta step of `kernel`.
    fn integrate(&self, kernel: impl Fn(V3) -> V3, co: V3, location: V3) -> V3 {
        let k0 = kernel(co);
        let k1 = kernel(add(co, scale(k0, 0.5)));
        let k2 = kernel(add(co, scale(k1, 0.5)));
        let k3 = kernel(sub(add(co, k2), location));
        let sum = add(add(k0, scale(k1, 2.0)), add(scale(k2, 2.0), k3));
        scale(sum, 1.0 / 6.0)
    }

    /// `BKE_kelvinlet_scale()`.
    pub fn scale(&self, co: V3, location: V3, _normal: V3) -> V3 {
        self.integrate(|p| self.scale_at(p, location), co, location)
    }

    /// `BKE_kelvinlet_twist()`.
    pub fn twist(&self, co: V3, location: V3, normal: V3) -> V3 {
        self.integrate(|p| self.twist_at(p, location, normal), co, location)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> KelvinletParams {
        /* Default `elastic_deform_volume_preservation` is 0.5. */
        KelvinletParams::new(1.0, 1.0, 1.0, 0.5)
    }

    #[test]
    fn init_constants() {
        let k = p();
        let a = 1.0 / (4.0 * std::f32::consts::PI);
        assert!((k.a - a).abs() < 1e-7);
        assert!((k.b - a / 2.0).abs() < 1e-7);
        assert!((k.c - 2.0 * (3.0 * a - a)).abs() < 1e-6);
        assert_eq!(k.radius_scaled, [1.0, 2.0, 4.0]);
    }

    #[test]
    fn grab_centre_follows_delta_and_decays() {
        let k = p();
        let d = [0.0, 0.0, 1.0];
        let at = |x: f32| k.grab([x, 0.0, 0.0], [0.0; 3], d)[2];
        /* Centre: kernel (a-b)/r + a r_s^2 / 2r^3 with r_e = r_s; c scales it. */
        let centre = k.grab([0.0; 3], [0.0; 3], d);
        assert!(centre[2] > 0.0 && centre[0] == 0.0 && centre[1] == 0.0);
        assert!(at(0.5) < centre[2] && at(1.0) < at(0.5) && at(4.0) < at(1.0) && at(4.0) > 0.0);
    }

    #[test]
    fn biscale_and_triscale_decay() {
        let k = p();
        let d = [1.0, 0.0, 0.0];
        let bi = |x: f32| k.grab_biscale([0.0, x, 0.0], [0.0; 3], d)[0];
        let tri = |x: f32| k.grab_triscale([0.0, x, 0.0], [0.0; 3], d)[0];
        assert!(bi(0.0) > bi(1.0) && bi(1.0) > bi(3.0) && bi(3.0) > 0.0);
        assert!(tri(0.0) > tri(1.0) && tri(1.0) > tri(3.0) && tri(3.0) > 0.0);
    }

    #[test]
    fn scale_is_radial_and_zero_at_centre() {
        let mut k = p();
        k.f = 1.0;
        assert_eq!(k.scale([0.0; 3], [0.0; 3], [0.0, 0.0, 1.0]), [0.0; 3]);
        let d = k.scale([1.0, 0.0, 0.0], [0.0; 3], [0.0, 0.0, 1.0]);
        assert!(d[0] != 0.0 && d[1].abs() < 1e-7 && d[2].abs() < 1e-7);
        k.f = -1.0;
        let n = k.scale([1.0, 0.0, 0.0], [0.0; 3], [0.0, 0.0, 1.0]);
        assert!(n[0] * d[0] < 0.0); /* the sign of the force flips the direction (RK4 is not exactly linear) */
    }

    #[test]
    fn twist_is_tangential_about_normal() {
        let mut k = p();
        k.f = 1.0;
        let d = k.twist([1.0, 0.0, 0.0], [0.0; 3], [0.0, 0.0, 1.0]);
        assert!(d[1].abs() > 0.0 && d[2].abs() < 1e-7);
        assert_eq!(k.twist([0.0; 3], [0.0; 3], [0.0, 0.0, 1.0]), [0.0; 3]);
    }
}
