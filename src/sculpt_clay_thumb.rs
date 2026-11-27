//! Clay Thumb brush (`mesh/brushes/clay_thumb.cc`: `do_clay_thumb_brush`, `calc_faces`,
//! `clay_thumb_get_stabilized_pressure`; `calc_translations_to_plane` in sculpt.cc). The brush plane through the dab
//! centre is tilted about the brush local X axis (`area_normal x grab_delta`) by `front_angle`, which grows by 0.8 degrees
//! per stroke step up to 60 (the simulated clay accumulation); vertices in the radius are pulled onto that plane by
//! `bstrength * pressure * falloff`. Deviations: only the Mesh PBVH path, the first dab of a symmetry pass is skipped by the caller.

use crate::mesh::{add, cross, dot, normalized, scale, sub, V3};

/// Increase of `clay_thumb_brush.front_angle` per main symmetry pass step.
pub const FRONT_ANGLE_STEP: f32 = 0.8;
/// Upper clamp of `front_angle` (degrees).
pub const FRONT_ANGLE_MAX: f32 = 60.0;

/// A plane `(normal, d)` with `dot(normal, p) + d == 0` on it (`plane_from_point_normal_v3`).
pub type Plane = (V3, f32);

/// `brush_strength()` for `SCULPT_BRUSH_TYPE_CLAY_THUMB`: `alpha * flip * pressure * overlap * 1.3` (pressure and overlap are 1).
pub fn brush_strength(alpha: f32, invert: bool) -> f32 {
    alpha * if invert { -1.0 } else { 1.0 } * 1.3
}

/// `front_angle += 0.8`, clamped to `0..=60`.
pub fn next_front_angle(angle: f32) -> f32 {
    (angle + FRONT_ANGLE_STEP).clamp(0.0, FRONT_ANGLE_MAX)
}

/// `clay_thumb_get_stabilized_pressure()`: mean of the pressure stabilizer buffer (0 when empty).
pub fn stabilized_pressure(buffer: &[f32]) -> f32 {
    if buffer.is_empty() {
        return 0.0;
    }
    buffer.iter().sum::<f32>() / buffer.len() as f32
}

/// `rotate_v3_v3v3fl()`: `v` turned about `axis` (normalized here) by `angle` radians.
fn rotate(v: V3, axis: V3, angle: f32) -> V3 {
    let axis = normalized(axis);
    let (s, c) = angle.sin_cos();
    add(add(scale(v, c), scale(cross(axis, v), s)), scale(axis, dot(axis, v) * (1.0 - c)))
}

/// The tilted front plane: `normal_tilt` is `plane_normal` rotated about the brush local X axis
/// (`normalize(plane_normal x grab_delta)` after `normalize_m4`) by `-front_angle` degrees. `None` for a zero delta
/// (the brush returns early) or a delta parallel to the normal.
pub fn tilted_plane(plane_normal: V3, location: V3, grab_delta: V3, front_angle_deg: f32) -> Option<Plane> {
    let x = cross(plane_normal, grab_delta);
    if dot(x, x) < 1e-24 {
        return None;
    }
    let tilt = rotate(plane_normal, x, -front_angle_deg.to_radians());
    Some((tilt, -dot(tilt, location)))
}

/// `calc_translations_to_plane()` for one position: `closest_to_plane_normalized_v3() - position`.
pub fn translation_to_plane(plane: &Plane, p: V3) -> V3 {
    scale(plane.0, -(dot(plane.0, p) + plane.1))
}

/// One dab: vertices inside `radius` of `center` move toward `plane` by `strength * falloff(dist / radius)`, masked.
#[allow(clippy::too_many_arguments)]
pub fn dab(verts: &mut [V3], mask: &[f32], plane: &Plane, center: V3, radius: f32, strength: f32, falloff: impl Fn(f32) -> f32) {
    for (i, v) in verts.iter_mut().enumerate() {
        let d = sub(*v, center);
        let dist = dot(d, d).sqrt();
        if dist >= radius {
            continue;
        }
        let factor = falloff(dist / radius) * (1.0 - mask.get(i).copied().unwrap_or(0.0)).clamp(0.0, 1.0) * strength;
        if factor == 0.0 {
            continue;
        }
        *v = add(*v, scale(translation_to_plane(plane, *v), factor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: V3 = [0.0, 0.0, 1.0];

    #[test]
    fn strength_is_alpha_times_1_3_flipped() {
        assert!((brush_strength(0.5, false) - 0.65).abs() < 1e-6);
        assert!((brush_strength(0.5, true) + 0.65).abs() < 1e-6);
    }

    #[test]
    fn front_angle_grows_and_clamps() {
        assert!((next_front_angle(0.0) - 0.8).abs() < 1e-6);
        assert_eq!(next_front_angle(59.9), 60.0);
        assert_eq!(next_front_angle(60.0), 60.0);
    }

    #[test]
    fn pressure_is_the_mean() {
        assert_eq!(stabilized_pressure(&[1.0, 0.0, 0.5, 0.5]), 0.5);
        assert_eq!(stabilized_pressure(&[]), 0.0);
    }

    #[test]
    fn zero_angle_plane_is_flat_through_centre() {
        let (n, d) = tilted_plane(N, [0.0, 0.0, 2.0], [1.0, 0.0, 0.0], 0.0).unwrap();
        assert!((n[2] - 1.0).abs() < 1e-6 && n[0].abs() < 1e-6);
        assert!((d + 2.0).abs() < 1e-6);
        assert!(tilted_plane(N, [0.0; 3], [0.0; 3], 10.0).is_none());
        assert!(tilted_plane(N, [0.0; 3], N, 10.0).is_none());
    }

    #[test]
    fn tilt_is_about_the_cross_axis_by_minus_the_angle() {
        /* x = N x (1,0,0) = (0,1,0); rotating +Z about +Y by -90 degrees gives -X. */
        let (n, _) = tilted_plane(N, [0.0; 3], [1.0, 0.0, 0.0], 90.0).unwrap();
        assert!((n[0] + 1.0).abs() < 1e-5 && n[2].abs() < 1e-5);
    }

    #[test]
    fn dab_pulls_onto_the_plane_scaled_by_falloff_and_mask() {
        let plane = tilted_plane(N, [0.0; 3], [1.0, 0.0, 0.0], 0.0).unwrap();
        let mut v = vec![[0.5, 0.0, 0.5], [0.5, 0.0, 0.5], [0.5, 0.0, -0.5], [3.0, 0.0, 1.0]];
        dab(&mut v, &[0.0, 1.0, 0.0, 0.0], &plane, [0.0; 3], 1.0, 0.5, |_| 1.0);
        assert!((v[0][2] - 0.25).abs() < 1e-6);
        assert_eq!(v[1], [0.5, 0.0, 0.5]);
        assert!((v[2][2] + 0.25).abs() < 1e-6);
        assert_eq!(v[3], [3.0, 0.0, 1.0]);
    }
}
