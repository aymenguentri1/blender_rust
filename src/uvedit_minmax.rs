//! Port of `ED_uvedit_minmax_multi()`, `uvedit_median_multi()` and `ED_uvedit_center_multi()`
//! from `source/blender/editors/uvedit/uvedit_ops.cc`. The `ED_uvedit_foreach_uv_multi()`
//! iteration is replaced by a slice of the already visible+selected UVs.

/// `INIT_MINMAX2` uses +/-1.0e30.
const MINMAX_INIT: f32 = 1.0e30;

/// `ED_uvedit_minmax_multi()`: returns `(min, max, changed)`.
pub fn uvedit_minmax(uvs: &[[f32; 2]]) -> ([f32; 2], [f32; 2], bool) {
    let mut min = [MINMAX_INIT; 2];
    let mut max = [-MINMAX_INIT; 2];
    let mut changed = false;
    for luv in uvs {
        for i in 0..2 {
            if luv[i] < min[i] {
                min[i] = luv[i];
            }
            if luv[i] > max[i] {
                max[i] = luv[i];
            }
        }
        changed = true;
    }
    (min, max, changed)
}

/// `uvedit_median_multi()`: returns `(co, found)`. Like Blender, `co` is NaN when empty
/// (`0 * (1/0)`), and callers must check the flag.
pub fn uvedit_median(uvs: &[[f32; 2]]) -> ([f32; 2], bool) {
    let mut co = [0.0f32; 2];
    let mut sel: u32 = 0;
    for luv in uvs {
        co[0] += luv[0];
        co[1] += luv[1];
        sel += 1;
    }
    let f = 1.0 / sel as f32;
    co[0] *= f;
    co[1] *= f;
    (co, sel != 0)
}

/// `V3D_AROUND_CENTER_BOUNDS` vs the remaining pivot modes (median).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CenterMode {
    Bounds,
    Median,
}

/// `ED_uvedit_center_multi()`: returns `Some(center)` when something was selected.
pub fn uvedit_center(uvs: &[[f32; 2]], mode: CenterMode) -> Option<[f32; 2]> {
    match mode {
        CenterMode::Bounds => {
            let (min, max, changed) = uvedit_minmax(uvs);
            changed.then(|| [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5])
        }
        CenterMode::Median => {
            let (co, found) = uvedit_median(uvs);
            found.then_some(co)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PTS: [[f32; 2]; 3] = [[0.0, 1.0], [4.0, -2.0], [2.0, 4.0]];

    #[test]
    fn minmax_of_known_set() {
        let (min, max, changed) = uvedit_minmax(&PTS);
        assert!(changed);
        assert_eq!((min, max), ([0.0, -2.0], [4.0, 4.0]));
    }

    #[test]
    fn minmax_empty_keeps_init() {
        let (min, max, changed) = uvedit_minmax(&[]);
        assert!(!changed);
        assert_eq!((min, max), ([1.0e30; 2], [-1.0e30; 2]));
    }

    #[test]
    fn median_is_mean() {
        let (co, found) = uvedit_median(&PTS);
        assert!(found);
        assert_eq!(co, [2.0, 1.0]);
    }

    #[test]
    fn median_empty_returns_false() {
        assert!(!uvedit_median(&[]).1);
    }

    #[test]
    fn center_modes() {
        assert_eq!(uvedit_center(&PTS, CenterMode::Bounds), Some([2.0, 1.0]));
        assert_eq!(uvedit_center(&[[0.0, 0.0], [1.0, 0.0], [5.0, 3.0]], CenterMode::Median), Some([2.0, 1.0]));
        assert_eq!(uvedit_center(&[[0.0, 0.0], [1.0, 0.0], [5.0, 3.0]], CenterMode::Bounds), Some([2.5, 1.5]));
        assert_eq!(uvedit_center(&[], CenterMode::Bounds), None);
        assert_eq!(uvedit_center(&[], CenterMode::Median), None);
    }
}
