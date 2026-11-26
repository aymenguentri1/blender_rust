//! Port of `applyTrackballValue_calc_axis_angle()` and the per-element matrix choice of
//! `transdata_elem_trackball()` in
//! `source/blender/editors/transform/transform_mode_trackball.cc`.

use crate::scene::{axis_angle_to_mat3, Mat3};
use crate::view3d::Mat4;

type V3 = [f32; 3];

/// `normalize_v3_v3()`: zero vector stays zero (returns the original length too).
fn normalize(v: V3) -> (V3, f32) {
    let d = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if d > 1.0e-35 {
        let l = d.sqrt();
        ([v[0] / l, v[1] / l, v[2] / l], l)
    } else {
        ([0.0; 3], 0.0)
    }
}

/// Axis and angle for the trackball values `phi` (radians about the view's first and second
/// axes): `axis = normalize(persinv[0]) * phi[0] + normalize(persinv[1]) * phi[1]`, then split
/// into a unit axis and its length as the angle.
pub fn calc_axis_angle(persinv: &Mat4, phi: [f32; 2]) -> (V3, f32) {
    let (a1, _) = normalize([persinv[0][0], persinv[0][1], persinv[0][2]]);
    let (a2, _) = normalize([persinv[1][0], persinv[1][1], persinv[1][2]]);
    let axis: V3 = std::array::from_fn(|i| a1[i] * phi[0] + a2[i] * phi[1]);
    normalize(axis)
}

/// Rotation matrix for one element: `td->factor * angle` under proportional editing, otherwise
/// the shared `angle`.
pub fn element_matrix(axis: V3, angle: f32, prop_edit: bool, factor: f32) -> Mat3 {
    axis_angle_to_mat3(axis, if prop_edit { factor * angle } else { angle })
}

#[cfg(test)]
mod tests {
    use super::*;

    const I: Mat4 = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

    #[test]
    fn zero_phi_is_zero_angle() {
        let (_, a) = calc_axis_angle(&I, [0.0, 0.0]);
        assert_eq!(a, 0.0);
    }

    #[test]
    fn phi_along_first_view_axis() {
        let mut m = I;
        m[0] = [3.0, 0.0, 0.0, 0.0]; // unnormalized row must be normalized
        let (axis, a) = calc_axis_angle(&m, [0.5, 0.0]);
        assert_eq!(axis, [1.0, 0.0, 0.0]);
        assert!((a - 0.5).abs() < 1e-6);
        let (axis, a) = calc_axis_angle(&m, [-0.5, 0.0]);
        assert_eq!(axis, [-1.0, 0.0, 0.0]);
        assert!((a - 0.5).abs() < 1e-6);
    }

    #[test]
    fn combined_axes_angle_is_the_length() {
        let (axis, a) = calc_axis_angle(&I, [0.3, 0.4]);
        assert!((a - 0.5).abs() < 1e-6);
        assert!((axis[0] - 0.6).abs() < 1e-6 && (axis[1] - 0.8).abs() < 1e-6 && axis[2] == 0.0);
    }

    #[test]
    fn matrix_is_orthonormal_and_factor_scales_angle() {
        let (axis, a) = calc_axis_angle(&I, [0.3, 0.4]);
        let m = element_matrix(axis, a, false, 0.25);
        for r in 0..3 {
            for c in 0..3 {
                let d: f32 = (0..3).map(|k| m[r][k] * m[c][k]).sum();
                assert!((d - if r == c { 1.0 } else { 0.0 }).abs() < 1e-5);
            }
        }
        assert_eq!(element_matrix(axis, a, true, 0.5), axis_angle_to_mat3(axis, 0.25));
        assert_eq!(element_matrix(axis, a, false, 0.5), axis_angle_to_mat3(axis, a));
    }
}
