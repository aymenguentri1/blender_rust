//! Port of `eUVWeldAlign` and `uvedit_uv_align_weld()` from
//! `source/blender/editors/uvedit/uvedit_ops.cc`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UvWeldAlign {
    Straighten,
    StraightenX,
    StraightenY,
    AlignAuto,
    AlignX,
    AlignY,
    Weld,
}

/// `uvedit_uv_align_weld()`: the callback body of `ED_uvedit_foreach_uv()` applied to the
/// (already visible+selected) UVs. Returns whether any coordinate changed.
pub fn uv_align_weld(uvs: &mut [[f32; 2]], tool: UvWeldAlign, cent: [f32; 2]) -> bool {
    let mut changed = false;
    for luv in uvs.iter_mut() {
        if matches!(tool, UvWeldAlign::AlignX | UvWeldAlign::Weld) && luv[0] != cent[0] {
            luv[0] = cent[0];
            changed = true;
        }
        if matches!(tool, UvWeldAlign::AlignY | UvWeldAlign::Weld) && luv[1] != cent[1] {
            luv[1] = cent[1];
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_x_only_changes_x() {
        let mut uvs = [[0.0, 1.0], [2.0, 3.0]];
        assert!(uv_align_weld(&mut uvs, UvWeldAlign::AlignX, [5.0, 9.0]));
        assert_eq!(uvs, [[5.0, 1.0], [5.0, 3.0]]);
    }

    #[test]
    fn align_y_only_changes_y() {
        let mut uvs = [[0.0, 1.0], [2.0, 3.0]];
        assert!(uv_align_weld(&mut uvs, UvWeldAlign::AlignY, [5.0, 9.0]));
        assert_eq!(uvs, [[0.0, 9.0], [2.0, 9.0]]);
    }

    #[test]
    fn weld_changes_both() {
        let mut uvs = [[0.0, 1.0], [2.0, 3.0]];
        assert!(uv_align_weld(&mut uvs, UvWeldAlign::Weld, [5.0, 9.0]));
        assert_eq!(uvs, [[5.0, 9.0], [5.0, 9.0]]);
    }

    #[test]
    fn unchanged_returns_false() {
        let mut uvs = [[5.0, 9.0]];
        assert!(!uv_align_weld(&mut uvs, UvWeldAlign::Weld, [5.0, 9.0]));
        assert!(!uv_align_weld(&mut [], UvWeldAlign::Weld, [5.0, 9.0]));
    }

    #[test]
    fn other_tools_do_nothing() {
        let mut uvs = [[0.0, 1.0]];
        for t in [UvWeldAlign::Straighten, UvWeldAlign::StraightenX, UvWeldAlign::StraightenY, UvWeldAlign::AlignAuto] {
            assert!(!uv_align_weld(&mut uvs, t, [5.0, 9.0]));
        }
        assert_eq!(uvs, [[0.0, 1.0]]);
    }
}
