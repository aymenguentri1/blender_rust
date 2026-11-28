//! Curves sculpt Delete brush (editors/sculpt_paint/curves/sculpt_delete.cc): curves touched by the brush are
//! removed. Ports `delete_projected()` (:150-195, screen space, `<=` radius) and `delete_spherical()` (:218-253,
//! 3D, `<` radius for single-point curves and `<=` for segments) as pure functions over curve point ranges.
//! Deviation: positions are already projected / transformed by the caller; symmetry copies are the caller
//! running these once per mirrored brush position and OR-ing the results.

type V2 = [f32; 2];
type V3 = [f32; 3];

/// `dist_squared_to_line_segment_v2()`.
pub fn dist_sq_to_segment_v2(p: V2, a: V2, b: V2) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let l2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if l2 == 0.0 { 0.0 } else { (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2).clamp(0.0, 1.0) };
    let (dx, dy) = (p[0] - (a[0] + ab[0] * t), p[1] - (a[1] + ab[1] * t));
    dx * dx + dy * dy
}

/// `dist_squared_to_line_segment_v3()`.
pub fn dist_sq_to_segment_v3(p: V3, a: V3, b: V3) -> f32 {
    let ab: V3 = std::array::from_fn(|i| b[i] - a[i]);
    let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if l2 == 0.0 { 0.0 } else { ((0..3).map(|i| (p[i] - a[i]) * ab[i]).sum::<f32>() / l2).clamp(0.0, 1.0) };
    (0..3).map(|i| { let d = p[i] - (a[i] + ab[i] * t); d * d }).sum()
}

/// `delete_projected()`: `keep[curve]` is cleared when a single point lies within `radius` of `brush` or any
/// segment passes within it. `points_by_curve` are the point ranges, `selected` the curve selection.
pub fn delete_projected(keep: &mut [bool], points_by_curve: &[std::ops::Range<usize>], selected: &[usize], pos: &[V2], brush: V2, radius: f32) {
    let r2 = radius * radius;
    for &c in selected {
        let pts = points_by_curve[c].clone();
        if pts.len() == 1 {
            let d = [pos[pts.start][0] - brush[0], pos[pts.start][1] - brush[1]];
            if d[0] * d[0] + d[1] * d[1] <= r2 { keep[c] = false; }
            continue;
        }
        for i in pts.start..pts.end - 1 {
            if dist_sq_to_segment_v2(brush, pos[i], pos[i + 1]) <= r2 { keep[c] = false; break; }
        }
    }
}

/// `delete_spherical()`: same with 3D distances; a single point must be strictly inside the radius.
pub fn delete_spherical(keep: &mut [bool], points_by_curve: &[std::ops::Range<usize>], selected: &[usize], pos: &[V3], brush: V3, radius: f32) {
    let r2 = radius * radius;
    for &c in selected {
        let pts = points_by_curve[c].clone();
        if pts.len() == 1 {
            let d2: f32 = (0..3).map(|i| { let d = pos[pts.start][i] - brush[i]; d * d }).sum();
            if d2 < r2 { keep[c] = false; }
            continue;
        }
        for i in pts.start..pts.end - 1 {
            if dist_sq_to_segment_v3(brush, pos[i], pos[i + 1]) <= r2 { keep[c] = false; break; }
        }
    }
}

/// Positions of the points of the curves still kept (`new_deformed_positions` in `execute()`).
pub fn kept_positions<T: Copy>(keep: &[bool], points_by_curve: &[std::ops::Range<usize>], pos: &[T]) -> Vec<T> {
    let mut out = Vec::new();
    for (c, pts) in points_by_curve.iter().enumerate() {
        if keep[c] { out.extend_from_slice(&pos[pts.clone()]); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_point_boundary_differs_between_modes() {
        let ranges = [0..1];
        let mut k = [true];
        delete_projected(&mut k, &ranges, &[0], &[[1.0, 0.0]], [0.0, 0.0], 1.0);
        assert!(!k[0]);
        let mut k = [true];
        delete_spherical(&mut k, &ranges, &[0], &[[1.0, 0.0, 0.0]], [0.0; 3], 1.0);
        assert!(k[0]);
        delete_spherical(&mut k, &ranges, &[0], &[[0.5, 0.0, 0.0]], [0.0; 3], 1.0);
        assert!(!k[0]);
    }

    #[test]
    fn segment_crossing_deletes_and_far_curve_is_kept() {
        let ranges = [0..3, 3..5];
        let pos = [[-5.0, 1.0], [5.0, 1.0], [9.0, 9.0], [20.0, 20.0], [30.0, 20.0]];
        let mut k = [true, true];
        delete_projected(&mut k, &ranges, &[0, 1], &pos, [0.0, 0.0], 1.5);
        assert_eq!(k, [false, true]);
    }

    #[test]
    fn unselected_curves_are_never_deleted() {
        let ranges = [0..2];
        let mut k = [true];
        delete_projected(&mut k, &ranges, &[], &[[0.0, 0.0], [1.0, 0.0]], [0.0, 0.0], 5.0);
        assert!(k[0]);
    }

    #[test]
    fn kept_positions_drop_deleted_curves() {
        let ranges = [0..2, 2..3, 3..5];
        let pos = [1, 2, 3, 4, 5];
        assert_eq!(kept_positions(&[true, false, true], &ranges, &pos), vec![1, 2, 4, 5]);
    }
}
