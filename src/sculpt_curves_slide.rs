//! Curves sculpt Slide brush (editors/sculpt_paint/curves/sculpt_slide.cc): curve roots slide over the surface
//! and the curve is rigidly moved with them. Ports the pure parts: the symmetry pre-pass
//! (`find_curves_to_slide_with_symmetry()`: each curve goes to the closest brush within the radius, ties keep the
//! first), the per-curve falloff, `find_closest_ray_hit()`, `get_slide_transform()` (rotation between old and
//! new normal about the root, then translate to the new root) and `rotation_between_vecs_to_mat3()`
//! (math_rotation_c.cc:493). Deviation: surface BVH, UV sampling and projection are done by the caller.

type V3 = [f32; 3];
type M3 = [[f32; 3]; 3];

fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn dist_sq(a: V3, b: V3) -> f32 { (0..3).map(|i| (a[i] - b[i]).powi(2)).sum() }

/// `ortho_v3_v3()`: a vector perpendicular to `v`.
fn ortho(v: V3) -> V3 {
    let (ax, ay, az) = (v[0].abs(), v[1].abs(), v[2].abs());
    if ax >= ay && ax >= az { [-v[1], v[0], 0.0] } else if ay >= az { [0.0, -v[2], v[1]] } else { [v[2], 0.0, -v[0]] }
}

fn normalized(v: V3) -> (V3, f32) {
    let l = dot(v, v).sqrt();
    if l == 0.0 { ([0.0; 3], 0.0) } else { ([v[0] / l, v[1] / l, v[2] / l], l) }
}

/// `axis_angle_normalized_to_mat3_ex()` in Blender's column-major storage (m[col][row]).
fn axis_angle(axis: V3, s: f32, c: f32) -> M3 {
    let (x, y, z) = (axis[0], axis[1], axis[2]);
    let ic = 1.0 - c;
    [
        [x * x * ic + c, x * y * ic + z * s, x * z * ic - y * s],
        [x * y * ic - z * s, y * y * ic + c, y * z * ic + x * s],
        [x * z * ic + y * s, y * z * ic - x * s, z * z * ic + c],
    ]
}

/// `rotation_between_vecs_to_mat3()`: unit vectors in, rotation taking `v1` to `v2`.
pub fn rotation_between(v1: V3, v2: V3) -> M3 {
    let (axis, s) = normalized(cross(v1, v2));
    let c = dot(v1, v2);
    if s > f32::EPSILON {
        axis_angle(axis, s, c)
    } else if c > 0.0 {
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    } else {
        axis_angle(normalized(ortho(v1)).0, 0.0, -1.0)
    }
}

/// `mul_m3_v3(m, v)` with column-major `m[col][row]` storage.
fn apply(m: &M3, v: V3) -> V3 {
    std::array::from_fn(|r| m[0][r] * v[0] + m[1][r] * v[1] + m[2][r] * v[2])
}

/// `get_slide_transform()` applied to a point: `rotation * (p - old_root) + new_root`.
pub fn slide_point(p: V3, old_root: V3, new_root: V3, old_normal: V3, new_normal: V3) -> V3 {
    let rot = rotation_between(old_normal, new_normal);
    let r = apply(&rot, std::array::from_fn(|i| p[i] - old_root[i]));
    std::array::from_fn(|i| r[i] + new_root[i])
}

/// Pre-pass: for every curve root, the index of the closest symmetry brush position within `radius`
/// (`dist_sq <= r^2`, strictly closer replaces), or -1.
pub fn best_brush(roots: &[V3], brushes: &[V3], radius: f32) -> Vec<i32> {
    let r2 = radius * radius;
    let mut best = vec![-1i32; roots.len()];
    let mut min = vec![f32::MAX; roots.len()];
    for (b, &bp) in brushes.iter().enumerate() {
        for (i, &root) in roots.iter().enumerate() {
            let d = dist_sq(root, bp);
            if d <= r2 && d < min[i] {
                min[i] = d;
                best[i] = b as i32;
            }
        }
    }
    best
}

/// `radius_falloff` of a curve root for its brush.
pub fn root_falloff(root: V3, brush: V3, radius: f32, falloff: impl Fn(f32, f32) -> f32) -> f32 {
    falloff(dist_sq(root, brush).sqrt(), radius)
}

/// Per-curve weight used to move the screen-space root: `strength * radius_falloff * curve_factor`.
pub fn curve_weight(strength: f32, radius_falloff: f32, curve_factor: f32) -> f32 {
    strength * radius_falloff * curve_factor
}

/// `find_closest_ray_hit()`: index of the hit nearest to `point`, first wins ties; `None` if empty.
pub fn closest_hit(hits: &[V3], point: V3) -> Option<usize> {
    let mut best = f32::MAX;
    let mut idx = None;
    for (i, &h) in hits.iter().enumerate() {
        let d = dist_sq(h, point);
        if d < best {
            best = d;
            idx = Some(i);
        }
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: V3, b: V3) -> bool { (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5) }

    #[test]
    fn same_normals_give_a_pure_translation() {
        let p = slide_point([1.0, 2.0, 3.0], [1.0, 0.0, 0.0], [5.0, 5.0, 5.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]);
        assert!(near(p, [5.0, 7.0, 8.0]));
    }

    #[test]
    fn root_maps_to_new_root_and_normal_rotates_to_new_normal() {
        let (n0, n1) = ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
        let root = [2.0, 0.0, 0.0];
        assert!(near(slide_point(root, root, [9.0, 9.0, 9.0], n0, n1), [9.0, 9.0, 9.0]));
        let tip = [root[0], root[1], root[2] + 1.0];
        assert!(near(slide_point(tip, root, [0.0; 3], n0, n1), [1.0, 0.0, 0.0]));
    }

    #[test]
    fn opposed_normals_rotate_by_180_degrees() {
        let p = slide_point([0.0, 0.0, 1.0], [0.0; 3], [0.0; 3], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]);
        assert!(near(p, [0.0, 0.0, -1.0]));
    }

    #[test]
    fn rotation_preserves_lengths() {
        let n1 = normalized([1.0, 2.0, 2.0]).0;
        let p = slide_point([0.3, -0.4, 0.9], [0.0; 3], [0.0; 3], [0.0, 0.0, 1.0], n1);
        assert!((dot(p, p) - dot([0.3, -0.4, 0.9], [0.3, -0.4, 0.9])).abs() < 1e-5);
    }

    #[test]
    fn best_brush_picks_closest_within_radius_and_first_on_tie() {
        let roots = [[0.0; 3], [10.0, 0.0, 0.0], [0.5, 0.0, 0.0]];
        let brushes = [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]];
        assert_eq!(best_brush(&roots, &brushes, 2.0), vec![0, -1, 0]);
    }

    #[test]
    fn closest_hit_and_weight() {
        let hits = [[0.0, 0.0, 5.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]];
        assert_eq!(closest_hit(&hits, [0.0; 3]), Some(1));
        assert_eq!(closest_hit(&[], [0.0; 3]), None);
        assert!((curve_weight(0.5, 0.5, 0.5) - 0.125).abs() < 1e-6);
        assert!((root_falloff([3.0, 4.0, 0.0], [0.0; 3], 10.0, |d, r| 1.0 - d / r) - 0.5).abs() < 1e-6);
    }
}
