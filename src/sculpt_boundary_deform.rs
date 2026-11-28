//! Sculpt boundary deformation kernels (`mesh/sculpt_boundary.cc`: `calc_bend_position`, `calc_slide_position`,
//! `calc_inflate_position`, `calc_grab_position`, `calc_twist_position`, `filter_uninitialized_verts`, and the shared
//! tail of `calc_*_mesh`; `sculpt.cc`: `check_vertex_pivot_symmetry`, `filter_verts_outside_symmetry_area`,
//! `filter_translations`; `math::rotate_around_axis`).
//! Deviation: geometry target on plain position slices; automasking factors, clip/lock and the cloth target are the caller's.

use crate::mesh::{add, scale, sub, V3};

/// `BOUNDARY_STEPS_NONE`.
const STEPS_NONE: i32 = -1;

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 0.0 { scale(a, 1.0 / l) } else { [0.0; 3] }
}

/// `rotate_v3_v3v3fl()` / `axis_angle_normalized_to_mat3`: rotate `v` around the (normalized here) axis by `angle`.
pub fn rotate_around_axis_origin(v: V3, axis: V3, angle: f32) -> V3 {
    let k = normalize(axis);
    let (s, c) = angle.sin_cos();
    add(add(scale(v, c), scale(cross(k, v), s)), scale(k, dot(k, v) * (1.0 - c)))
}

/// `math::rotate_around_axis(vector, center, axis, angle)`.
pub fn rotate_around_axis(v: V3, center: V3, axis: V3, angle: f32) -> V3 {
    add(rotate_around_axis_origin(sub(v, center), axis, angle), center)
}

/// `filter_uninitialized_verts()`.
pub fn filter_uninitialized_verts(propagation_steps: &[i32], factors: &mut [f32]) {
    for (f, &s) in factors.iter_mut().zip(propagation_steps) {
        if s == STEPS_NONE {
            *f = 0.0;
        }
    }
}

/// `check_vertex_pivot_symmetry()`; `symm` bit `i` is the X/Y/Z mirror.
pub fn check_vertex_pivot_symmetry(vco: V3, pco: V3, symm: [bool; 3]) -> bool {
    let mut inside = true;
    for i in 0..3 {
        if symm[i] {
            if pco[i] == 0.0 && vco[i] > 0.0 {
                inside = false;
            }
            if vco[i] * pco[i] < 0.0 {
                inside = false;
            }
        }
    }
    inside
}

/// `filter_verts_outside_symmetry_area()`.
pub fn filter_verts_outside_symmetry_area(positions: &[V3], pivot: V3, symm: [bool; 3], factors: &mut [f32]) {
    for (f, &p) in factors.iter_mut().zip(positions) {
        if !check_vertex_pivot_symmetry(p, pivot, symm) {
            *f = 0.0;
        }
    }
}

/// The factor preparation shared by every `calc_*_mesh`: steps filter, symmetry filter, then `scale_factors`.
pub fn prepare_factors(
    factors: &mut [f32],
    propagation_steps: &[i32],
    orig_positions: &[V3],
    symmetry_pivot: V3,
    symm: [bool; 3],
    strength: f32,
) {
    filter_uninitialized_verts(propagation_steps, factors);
    filter_verts_outside_symmetry_area(orig_positions, symmetry_pivot, symm, factors);
    for f in factors.iter_mut() {
        *f *= strength;
    }
}

/// `translations_from_new_positions()` followed by `filter_translations()`.
pub fn translations(new_positions: &[V3], current: &[V3], factors: &[f32]) -> Vec<V3> {
    new_positions
        .iter()
        .zip(current)
        .zip(factors)
        .map(|((n, c), &f)| if f == 0.0 { [0.0; 3] } else { sub(*n, *c) })
        .collect()
}

/// `calc_bend_position()`.
pub fn bend(positions: &[V3], pivot_positions: &[V3], pivot_axes: &[V3], factors: &[f32]) -> Vec<V3> {
    (0..positions.len())
        .map(|i| add(rotate_around_axis_origin(sub(positions[i], pivot_positions[i]), pivot_axes[i], factors[i]), pivot_positions[i]))
        .collect()
}

/// `calc_slide_position()`.
pub fn slide(positions: &[V3], directions: &[V3], factors: &[f32]) -> Vec<V3> {
    (0..positions.len()).map(|i| add(positions[i], scale(directions[i], factors[i]))).collect()
}

/// `calc_inflate_position()`.
pub fn inflate(positions: &[V3], normals: &[V3], factors: &[f32]) -> Vec<V3> {
    (0..positions.len()).map(|i| add(positions[i], scale(normals[i], factors[i]))).collect()
}

/// `calc_grab_position()`.
pub fn grab(positions: &[V3], grab_delta: V3, factors: &[f32]) -> Vec<V3> {
    (0..positions.len()).map(|i| add(positions[i], scale(grab_delta, factors[i]))).collect()
}

/// `calc_twist_position()`.
pub fn twist(positions: &[V3], pivot_point: V3, pivot_axis: V3, factors: &[f32]) -> Vec<V3> {
    positions.iter().zip(factors).map(|(&p, &f)| rotate_around_axis(p, pivot_point, pivot_axis, f)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: V3, b: V3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5)
    }

    #[test]
    fn rotation_quarter_turn_about_z() {
        let r = rotate_around_axis([1.0, 0.0, 0.0], [0.0; 3], [0.0, 0.0, 2.0], std::f32::consts::FRAC_PI_2);
        assert!(near(r, [0.0, 1.0, 0.0]));
        let r = rotate_around_axis([2.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], std::f32::consts::PI);
        assert!(near(r, [0.0, 0.0, 0.0]));
    }

    #[test]
    fn uninitialized_and_symmetry_filters_zero_factors() {
        let mut f = vec![1.0, 1.0, 1.0];
        let pos = [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        prepare_factors(&mut f, &[0, 0, STEPS_NONE], &pos, [1.0, 0.0, 0.0], [true, false, false], 0.5);
        assert_eq!(f, vec![0.5, 0.0, 0.0]); // second is across the pivot, third has no steps
    }

    #[test]
    fn symmetry_pivot_on_the_plane_keeps_only_the_negative_side() {
        assert!(check_vertex_pivot_symmetry([-1.0, 0.0, 0.0], [0.0; 3], [true, false, false]));
        assert!(!check_vertex_pivot_symmetry([1.0, 0.0, 0.0], [0.0; 3], [true, false, false]));
        assert!(check_vertex_pivot_symmetry([1.0, 0.0, 0.0], [0.0; 3], [false; 3]));
    }

    #[test]
    fn bend_rotates_about_each_pivot() {
        let out = bend(&[[1.0, 0.0, 0.0]], &[[0.0; 3]], &[[0.0, 0.0, 1.0]], &[std::f32::consts::FRAC_PI_2]);
        assert!(near(out[0], [0.0, 1.0, 0.0]));
    }

    #[test]
    fn slide_inflate_grab_scale_by_factor() {
        let p = [[1.0, 1.0, 1.0]];
        assert!(near(slide(&p, &[[1.0, 0.0, 0.0]], &[0.5])[0], [1.5, 1.0, 1.0]));
        assert!(near(inflate(&p, &[[0.0, 0.0, 1.0]], &[2.0])[0], [1.0, 1.0, 3.0]));
        assert!(near(grab(&p, [0.0, 2.0, 0.0], &[0.25])[0], [1.0, 1.5, 1.0]));
    }

    #[test]
    fn twist_rotates_around_the_pivot_point() {
        let out = twist(&[[2.0, 0.0, 0.0]], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], &[std::f32::consts::FRAC_PI_2]);
        assert!(near(out[0], [1.0, 1.0, 0.0]));
    }

    #[test]
    fn translations_are_zero_where_the_factor_is_zero() {
        let t = translations(&[[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], &[[0.0; 3], [0.0; 3]], &[1.0, 0.0]);
        assert_eq!(t, vec![[1.0, 0.0, 0.0], [0.0; 3]]);
    }
}
