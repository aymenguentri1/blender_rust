//! Multi-plane Scrape brush, standard mode (`mesh/brushes/multiplane_scrape.cc`: `calc_translations`,
//! `filter_plane_side_factors`, `calc_distances`, `do_multiplane_scrape_brush`). Two planes through the brush centre,
//! each tilted by half the angle about the brush local Y axis (local X is
//! `area_normal x stroke_delta`), pull the vertices above them down onto them.
//! Deviation: the Dynamic mode (planes fitted to the surface on both sides) and the plane trim limit are not ported.

use crate::mesh::{add, cross, dot, normalized, scale, sub, V3};

/// `DNA_brush_types.h`: `multiplane_scrape_angle` has no non-zero default in the DNA, so the brush is a plain Scrape
/// until the angle is set.
pub const DEFAULT_ANGLE: f32 = 0.0;

/// A plane `(normal, d)` with `dot(normal, p) + d == 0` on it (`plane_from_point_normal_v3`).
type Plane = (V3, f32);

fn plane_from_point_normal(p: V3, n: V3) -> Plane {
    (n, -dot(n, p))
}

/// `plane_point_side_v3()`.
fn point_side(pl: &Plane, p: V3) -> f32 {
    dot(pl.0, p) + pl.1
}

/// `rotate_v3_v3v3fl()`: `v` turned about the unit `axis` by `angle` radians (Rodrigues).
fn rotate(v: V3, axis: V3, angle: f32) -> V3 {
    let (s, c) = angle.sin_cos();
    add(add(scale(v, c), scale(cross(axis, v), s)), scale(axis, dot(axis, v) * (1.0 - c)))
}

/// The brush local frame of `do_multiplane_scrape_brush()`: x `area_normal x delta`, y `area_normal x x`, z `area_normal`,
/// each normalized. `None` when the delta is parallel to the normal.
fn frame(area_normal: V3, delta: V3) -> Option<[V3; 3]> {
    let x = cross(area_normal, delta);
    let y = cross(area_normal, x);
    let len = |v: V3| dot(v, v).sqrt();
    if len(x) < 1e-12 || len(y) < 1e-12 {
        return None;
    }
    Some([normalized(x), normalized(y), normalized(area_normal)])
}

/// The scrape planes `[plane 0 (local x > 0), plane 1 (local x <= 0)]` through `position`.
pub fn scrape_planes(area_normal: V3, position: V3, frame_axes: &[V3; 3], angle_deg: f32) -> [Plane; 2] {
    let half = angle_deg.to_radians() * 0.5;
    let y_axis = [0.0, 1.0, 0.0];
    /* The normal in the local frame is (0, 0, 1) for a unit `area_normal`. */
    let local = [dot(area_normal, frame_axes[0]), dot(area_normal, frame_axes[1]), dot(area_normal, frame_axes[2])];
    let plane = |a: f32| {
        let r = rotate(local, y_axis, a);
        let world = add(add(scale(frame_axes[0], r[0]), scale(frame_axes[1], r[1])), scale(frame_axes[2], r[2]));
        plane_from_point_normal(position, normalized(world))
    };
    [plane(half), plane(-half)]
}

/// One dab: `verts` under the brush are moved onto the plane of their side. `delta` is the stroke direction,
/// `flip` the inverted brush (negative strength: radius and angle sign flip).
#[allow(clippy::too_many_arguments)]
pub fn dab(verts: &mut [V3], mask: &[f32], area_normal: V3, center: V3, delta: V3, radius: f32, strength: f32, angle_deg: f32, flip: bool, falloff: impl Fn(f32) -> f32) {
    if delta == [0.0; 3] {
        return;
    }
    let Some(axes) = frame(area_normal, delta) else { return };
    let angle = if flip { -angle_deg } else { angle_deg };
    let planes = scrape_planes(normalized(area_normal), center, &axes, angle);
    for (i, v) in verts.iter_mut().enumerate() {
        let d = sub(*v, center);
        let local = [dot(d, axes[0]), dot(d, axes[1]), dot(d, axes[2])];
        /* `calc_distances()`: the local space is stretched along Y. */
        let dist = (local[0] * local[0] + 4.0 * local[1] * local[1] + local[2] * local[2]).sqrt();
        if dist >= radius {
            continue;
        }
        let pl = &planes[(local[0] <= 0.0) as usize];
        let side = point_side(pl, *v);
        /* `filter_plane_side_factors()` (angle >= 0): only what is above the plane is scraped. */
        if angle >= 0.0 && side <= 0.0 {
            continue;
        }
        let m = mask.get(i).copied().unwrap_or(0.0);
        let f = falloff(dist / radius) * (1.0 - m).clamp(0.0, 1.0) * strength.abs();
        /* `closest_to_plane_normalized_v3()`: the translation is `-side * normal`. */
        *v = sub(*v, scale(pl.0, side * f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const Z: V3 = [0.0, 0.0, 1.0];
    const ONE: fn(f32) -> f32 = |_| 1.0;

    #[test]
    fn zero_angle_planes_are_flat_through_centre() {
        let axes = frame(Z, [1.0, 0.0, 0.0]).unwrap();
        let [p0, p1] = scrape_planes(Z, [0.0, 0.0, 0.5], &axes, 0.0);
        assert!((point_side(&p0, [3.0, 2.0, 0.5])).abs() < 1e-6 && (point_side(&p1, [-3.0, 1.0, 0.5])).abs() < 1e-6);
        assert!(point_side(&p0, [0.0, 0.0, 1.0]) > 0.0);
    }

    #[test]
    fn angled_planes_tilt_in_opposite_directions_across_x_local() {
        /* delta along +x: local x = n x delta = +y world. */
        let axes = frame(Z, [1.0, 0.0, 0.0]).unwrap();
        let [p0, p1] = scrape_planes(Z, [0.0; 3], &axes, 90.0);
        /* 45 degree planes: height of the plane at a distance 1 along local x is +-1, mirrored. */
        let h = |pl: &Plane, y: f32| -(pl.0[1] * y) / pl.0[2];
        assert!((h(&p0, 1.0).abs() - 1.0).abs() < 1e-5);
        assert!((h(&p0, 1.0) + h(&p1, 1.0)).abs() < 1e-5);
        assert!(h(&p0, 1.0) != 0.0);
    }

    #[test]
    fn dab_flattens_above_plane_and_leaves_below() {
        let mut v = vec![[0.0, 0.0, 0.2], [0.1, 0.1, -0.2], [0.1, 0.0, 0.1], [5.0, 0.0, 0.3]];
        dab(&mut v, &[], Z, [0.0; 3], [1.0, 0.0, 0.0], 1.0, 1.0, 0.0, false, ONE);
        assert!(v[0][2].abs() < 1e-6 && v[2][2].abs() < 1e-6);
        assert_eq!(v[1][2], -0.2); /* below the plane */
        assert_eq!(v[3][2], 0.3); /* outside the radius */
    }

    #[test]
    fn mask_strength_and_zero_delta() {
        let mut v = vec![[0.0, 0.0, 0.2], [0.1, 0.0, 0.2]];
        dab(&mut v, &[1.0, 0.0], Z, [0.0; 3], [1.0, 0.0, 0.0], 1.0, 0.5, 0.0, false, ONE);
        assert_eq!(v[0][2], 0.2);
        assert!((v[1][2] - 0.1).abs() < 1e-6);
        let before = v.clone();
        dab(&mut v, &[], Z, [0.0; 3], [0.0; 3], 1.0, 1.0, 0.0, false, ONE);
        assert_eq!(v, before);
    }
}
