//! Meeting point of two offset edge lines at a bevel vertex (edit-mode bevel, Ctrl+B).
//!
//! Ported from `bmesh/tools/bmesh_bevel.cc`, `offset_meet()` (1882) for the shared-face case (`edges_between == false`)
//! with the plain Offset width type, plus `isect_line_line_v3()` (math_geom). Not ported: the Percent/Absolute
//! line variant, the `edges_between` drop-to-face branch, `is_outside_edge` clamp for zero offsets.

use crate::mesh::{add, cross, dot, normalized, scale, sub, V3};

const EPSILON_ANG: f32 = 2.0 * std::f32::consts::PI / 180.0;
const SMALL_ANG: f32 = 10.0 * std::f32::consts::PI / 180.0;

fn angle(a: V3, b: V3) -> f32 {
    let (la, lb) = (dot(a, a).sqrt(), dot(b, b).sqrt());
    if la == 0.0 || lb == 0.0 {
        return 0.0;
    }
    (dot(a, b) / (la * lb)).clamp(-1.0, 1.0).acos()
}

/// `isect_line_line_v3`: 0 collinear/degenerate, 1 lines meet (`.1 == .2`), 2 skew (closest points on each line).
pub fn isect_line_line(v1: V3, v2: V3, v3: V3, v4: V3) -> (u8, V3, V3) {
    let (a, b, c) = (sub(v2, v1), sub(v4, v3), sub(v3, v1));
    let ab = cross(a, b);
    let div = dot(ab, ab);
    if div == 0.0 {
        return (0, v1, v1);
    }
    let t = dot(cross(c, b), ab) / div;
    let i1 = add(v1, scale(a, t));
    if dot(c, ab).abs() <= 0.000001 * div.abs() {
        return (1, i1, i1);
    }
    let s = dot(cross(c, a), ab) / div;
    (2, i1, add(v3, scale(b, s)))
}

/// One offset edge as seen from the bevel vertex.
#[derive(Clone, Copy, Debug)]
pub struct MeetEdge {
    /// Position of the edge's other vertex.
    pub other: V3,
    /// `offset_r` for `e1`, `offset_l` for `e2`.
    pub offset: f32,
}

/// `offset_meet()` with `edges_between == false`: `e1` precedes `e2` CCW around `v` (viewed from the normal side) and
/// both lie on face normal `f_no` (`None`: use the vertex normal `v_no`).
pub fn offset_meet(e1: MeetEdge, e2: MeetEdge, v: V3, v_no: V3, f_no: Option<V3>) -> V3 {
    let dir1 = sub(v, e1.other);
    let dir2 = sub(e2.other, v);
    let ang = angle(dir1, dir2);
    if ang < EPSILON_ANG {
        let norm_v = f_no.unwrap_or(v_no);
        let perp = normalized(cross(add(dir1, dir2), norm_v));
        let d = e1.offset.max(e2.offset) / (ang / 2.0).cos();
        return add(v, scale(perp, d));
    }
    if (ang - std::f32::consts::PI).abs() < EPSILON_ANG {
        let d = e1.offset.max(e2.offset);
        let to = sub(e2.other, v);
        let len = dot(to, to).sqrt();
        // slide_dist: along e2 from v, clamped to the edge end.
        return if len <= d { e2.other } else { add(v, scale(to, d / len)) };
    }
    let (norm_v1, norm_v2);
    if let (Some(n), true) = (f_no, ang < SMALL_ANG) {
        norm_v1 = n;
        norm_v2 = norm_v1;
    } else {
        let mut n = normalized(cross(dir2, dir1));
        if dot(n, f_no.unwrap_or(v_no)) < 0.0 {
            n = scale(n, -1.0);
        }
        norm_v1 = n;
        norm_v2 = n;
    }
    let p1 = normalized(cross(dir1, norm_v1));
    let p2 = normalized(cross(dir2, norm_v2));
    let off1a = add(v, scale(p1, e1.offset));
    let off1b = add(off1a, dir1);
    let off2a = add(v, scale(p2, e2.offset));
    let off2b = add(off2a, dir2);
    let (kind, meet, _) = isect_line_line(off1a, off1b, off2a, off2b);
    if kind == 0 {
        off1a
    } else {
        meet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const Z: V3 = [0.0, 0.0, 1.0];

    fn close(a: V3, b: V3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5)
    }

    #[test]
    fn right_angle_corner_meets_on_the_bisector() {
        // Corner at the origin, e1 along +x then e2 along +y (CCW from +z): the face is the first quadrant.
        let e1 = MeetEdge { other: [1.0, 0.0, 0.0], offset: 0.2 };
        let e2 = MeetEdge { other: [0.0, 1.0, 0.0], offset: 0.2 };
        let m = offset_meet(e1, e2, [0.0; 3], Z, Some(Z));
        assert!(close(m, [0.2, 0.2, 0.0]), "{m:?}");
    }

    #[test]
    fn unequal_offsets_intersect_the_offset_lines() {
        let e1 = MeetEdge { other: [1.0, 0.0, 0.0], offset: 0.1 };
        let e2 = MeetEdge { other: [0.0, 1.0, 0.0], offset: 0.3 };
        let m = offset_meet(e1, e2, [0.0; 3], Z, Some(Z));
        assert!(close(m, [0.3, 0.1, 0.0]), "{m:?}");
    }

    #[test]
    fn collinear_edges_offset_perpendicular() {
        let e1 = MeetEdge { other: [-1.0, 0.0, 0.0], offset: 0.25 };
        let e2 = MeetEdge { other: [1.0, 0.0, 0.0], offset: 0.25 };
        let m = offset_meet(e1, e2, [0.0; 3], Z, Some(Z));
        assert!(close(m, [0.0, -0.25, 0.0]) || close(m, [0.0, 0.25, 0.0]), "{m:?}");
    }

    #[test]
    fn line_intersection_kinds() {
        let (k, p, _) = isect_line_line([0.0; 3], [1.0, 0.0, 0.0], [2.0, -1.0, 0.0], [2.0, 1.0, 0.0]);
        assert_eq!(k, 1);
        assert!(close(p, [2.0, 0.0, 0.0]));
        let (k, ..) = isect_line_line([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]);
        assert_eq!(k, 0);
        let (k, a, b) = isect_line_line([0.0; 3], [1.0, 0.0, 0.0], [2.0, -1.0, 1.0], [2.0, 1.0, 1.0]);
        assert_eq!(k, 2);
        assert!(close(a, [2.0, 0.0, 0.0]) && close(b, [2.0, 0.0, 1.0]));
    }
}
