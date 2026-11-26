//! Port of `eUVEndPointPrecedence`, `uvedit_line_update_get_precedence()` and
//! `uvedit_line_update_endpoint()` from `source/blender/editors/uvedit/uvedit_ops.cc`.

/// `eUVEndPointPrecedence`: bitwise-or'd flags, the loop with the highest value wins.
pub const UVEP_INVALID: u8 = 0;
pub const UVEP_SELECTED: u8 = 1 << 0;
/// Pinned verts are preferred to selected.
pub const UVEP_PINNED: u8 = 1 << 1;

pub fn line_update_get_precedence(pinned: bool) -> u8 {
    let mut precedence = UVEP_SELECTED;
    if pinned {
        precedence |= UVEP_PINNED;
    }
    precedence
}

fn len_sq(a: [f32; 2], b: [f32; 2]) -> f32 {
    let (x, y) = (a[0] - b[0], a[1] - b[1]);
    x * x + y * y
}

/// `uvedit_line_update_endpoint()`: heuristic to find two high-precedence, far apart endpoints.
/// Returns true when `uv_a` or `uv_b` was replaced by `luv`.
pub fn line_update_endpoint(
    luv: [f32; 2],
    pinned: bool,
    uv_a: &mut [f32; 2],
    prec_a: &mut u8,
    uv_b: &mut [f32; 2],
    prec_b: &mut u8,
) -> bool {
    let flags = line_update_get_precedence(pinned);

    let len_sq_a = len_sq(*uv_a, luv);
    let len_sq_b = len_sq(*uv_b, luv);
    let len_sq_ab = len_sq(*uv_a, *uv_b);

    if (*prec_a < flags && 0.0 < len_sq_b) || (*prec_a == flags && len_sq_ab < len_sq_b) {
        *prec_a = flags;
        *uv_a = luv;
        return true;
    }
    if (*prec_b < flags && 0.0 < len_sq_a) || (*prec_b == flags && len_sq_ab < len_sq_a) {
        *prec_b = flags;
        *uv_b = luv;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_flags() {
        assert_eq!(line_update_get_precedence(false), 1);
        assert_eq!(line_update_get_precedence(true), 3);
    }

    #[test]
    fn first_point_replaces_invalid_a() {
        let (mut a, mut b) = ([0.0, 0.0], [0.0, 0.0]);
        let (mut pa, mut pb) = (UVEP_INVALID, UVEP_INVALID);
        // luv coincides with both points: nothing is replaced.
        assert!(!line_update_endpoint([0.0, 0.0], false, &mut a, &mut pa, &mut b, &mut pb));
        assert!(line_update_endpoint([1.0, 0.0], false, &mut a, &mut pa, &mut b, &mut pb));
        assert_eq!((a, pa), ([1.0, 0.0], UVEP_SELECTED));
    }

    #[test]
    fn pinned_beats_selected() {
        let (mut a, mut b) = ([0.0, 0.0], [4.0, 0.0]);
        let (mut pa, mut pb) = (UVEP_SELECTED, UVEP_SELECTED);
        assert!(line_update_endpoint([1.0, 0.0], true, &mut a, &mut pa, &mut b, &mut pb));
        assert_eq!((a, pa), ([1.0, 0.0], 3));
        assert_eq!((b, pb), ([4.0, 0.0], UVEP_SELECTED));
    }

    #[test]
    fn farther_point_replaces_closer_on_tie() {
        let (mut a, mut b) = ([0.0, 0.0], [1.0, 0.0]);
        let (mut pa, mut pb) = (UVEP_SELECTED, UVEP_SELECTED);
        // |ab|^2 = 1 < |b-luv|^2 = 25 -> replace a.
        assert!(line_update_endpoint([6.0, 0.0], false, &mut a, &mut pa, &mut b, &mut pb));
        assert_eq!(a, [6.0, 0.0]);
        assert_eq!(b, [1.0, 0.0]);
    }

    #[test]
    fn closer_point_is_ignored() {
        let (mut a, mut b) = ([0.0, 0.0], [10.0, 0.0]);
        let (mut pa, mut pb) = (UVEP_SELECTED, UVEP_SELECTED);
        assert!(!line_update_endpoint([5.0, 0.0], false, &mut a, &mut pa, &mut b, &mut pb));
        assert_eq!((a, b), ([0.0, 0.0], [10.0, 0.0]));
    }

    #[test]
    fn lower_precedence_never_replaces() {
        let (mut a, mut b) = ([0.0, 0.0], [1.0, 0.0]);
        let (mut pa, mut pb) = (3, 3);
        assert!(!line_update_endpoint([9.0, 0.0], false, &mut a, &mut pa, &mut b, &mut pb));
    }
}
