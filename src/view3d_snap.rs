//! Snap Cursor to Selected maths (`space_view3d/view3d_snap.cc`: `snap_curs_to_sel_ex`). The target is the centroid of
//! the selected points (accumulated in `double3` like Blender) or, for `V3D_AROUND_CENTER_BOUNDS`, the middle of their
//! bounding box (`mid_v3_v3v3(min, max)`).

use crate::mesh::V3;

/// `snap_curs_to_sel_ex()` for already collected world positions: `None` when `count == 0` (operator cancelled).
pub fn cursor_to_selected(points: &[V3], bounds: bool) -> Option<V3> {
    if points.is_empty() {
        return None;
    }
    if bounds {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in points {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        return Some(std::array::from_fn(|k| (min[k] + max[k]) * 0.5));
    }
    let mut sum = [0.0f64; 3];
    for p in points {
        for k in 0..3 {
            sum[k] += p[k] as f64;
        }
    }
    let n = points.len() as f64;
    Some(std::array::from_fn(|k| (sum[k] / n) as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_selection_cancels() {
        assert_eq!(cursor_to_selected(&[], false), None);
        assert_eq!(cursor_to_selected(&[], true), None);
    }

    #[test]
    fn median_is_the_centroid() {
        let p = [[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [0.0, 3.0, 6.0]];
        assert_eq!(cursor_to_selected(&p, false), Some([1.0, 1.0, 2.0]));
    }

    #[test]
    fn bounds_is_the_box_middle() {
        let p = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [10.0, 4.0, -2.0]];
        assert_eq!(cursor_to_selected(&p, true), Some([5.0, 2.0, -1.0]));
    }
}
