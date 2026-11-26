//! Graph editor: set the handle type of the selected keyframes (`GRAPH_OT_handle_type`).
//!
//! Ported from `sethandles_graph_keys()` (editors/space_graph/graph_edit.cc:1702), the `set_bezier_auto`,
//! `set_bezier_auto_clamped`, `set_bezier_vector`, `set_bezier_align`, `set_bezier_free` callbacks and
//! `ANIM_editkeyframes_handles()` (editors/animation/keyframes_edit.cc:1040-1190), and
//! `BKE_fcurve_update_handle_flag_from_opposite()` (blenkernel/intern/fcurve.cc:1263).
//! Enum values are `eBezTriple_Handle` (makesdna/DNA_curve_enums.h:181).
//! Not ported: `BKE_fcurve_handles_recalc` (handle positions are not recomputed), `bezier_isfree` (the unused
//! toggle), the F-curve / visibility filter, and moving keys and handles (transform_convert_graph.cc).

use crate::keyframe_select::BEZT_FLAG_SELECT;

/// `eBezTriple_Handle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HandleType {
    #[default]
    Free = 0,
    Auto = 1,
    Vector = 2,
    Align = 3,
    AutoClamped = 4,
    AlignDoubleSide = 5,
}

/// The handle types and selection flags of a `BezTriple` (`h1`, `h2`, `f1`, `f2`, `f3`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HandleFlags {
    pub h1: HandleType,
    pub h2: HandleType,
    pub f1: u8,
    pub f2: u8,
    pub f3: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl HandleFlags {
    fn sel(f: u8) -> bool {
        f & BEZT_FLAG_SELECT != 0
    }
    /// `BEZT_ISSEL_ANY`.
    pub fn is_sel_any(&self) -> bool {
        Self::sel(self.f1) || Self::sel(self.f2) || Self::sel(self.f3)
    }

    /// `BKE_fcurve_update_handle_flag_from_opposite`: keep the other handle's type valid after `side` changed.
    pub fn update_from_opposite(&mut self, side: Side) {
        use HandleType::*;
        let (source, target) = match side {
            Side::Left => (self.h1, &mut self.h2),
            Side::Right => (self.h2, &mut self.h1),
        };
        match source {
            Auto | AutoClamped | AlignDoubleSide => *target = source,
            Align => {
                if *target != Vector {
                    *target = source;
                }
            }
            Vector => {
                if !matches!(*target, Vector | Align) {
                    *target = Free;
                }
            }
            Free => {
                if *target != Vector {
                    *target = Free;
                }
            }
        }
    }

    /// `ANIM_editkeyframes_handles(mode)` applied to one key.
    pub fn set_type(&mut self, mode: HandleType) {
        match mode {
            // Both handles must be the same for auto types.
            HandleType::Auto | HandleType::AutoClamped => {
                if self.is_sel_any() {
                    self.h1 = mode;
                    self.h2 = mode;
                }
            }
            HandleType::Vector | HandleType::Align | HandleType::Free => {
                if Self::sel(self.f2) {
                    self.h1 = mode;
                    self.h2 = mode;
                } else {
                    if Self::sel(self.f1) {
                        self.h1 = mode;
                        self.update_from_opposite(Side::Left);
                    }
                    if Self::sel(self.f3) {
                        self.h2 = mode;
                        self.update_from_opposite(Side::Right);
                    }
                }
            }
            // `bezier_isfree` (default case) changes nothing.
            HandleType::AlignDoubleSide => {}
        }
    }
}

/// `sethandles_graph_keys` for one curve: only when some key is selected, change the type of the selected
/// handles. Returns whether the curve was changed (`ale.update`).
pub fn set_handle_type(keys: &mut [HandleFlags], mode: HandleType) -> bool {
    if !keys.iter().any(|k| k.is_sel_any()) {
        return false;
    }
    for k in keys.iter_mut() {
        k.set_type(mode);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use HandleType::*;

    const S: u8 = BEZT_FLAG_SELECT;
    fn key(h1: HandleType, h2: HandleType, f1: u8, f2: u8, f3: u8) -> HandleFlags {
        HandleFlags { h1, h2, f1, f2, f3 }
    }

    #[test]
    fn enum_values_match_dna() {
        assert_eq!(
            [Free as u8, Auto as u8, Vector as u8, Align as u8, AutoClamped as u8, AlignDoubleSide as u8],
            [0, 1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn auto_sets_both_when_anything_selected() {
        let mut k = key(Free, Vector, S, 0, 0);
        k.set_type(Auto);
        assert_eq!((k.h1, k.h2), (Auto, Auto));
        let mut k = key(Free, Free, S, S, S);
        k.set_type(AutoClamped);
        assert_eq!((k.h1, k.h2), (AutoClamped, AutoClamped));
        let mut k = key(Free, Vector, 0, 0, 0);
        k.set_type(Auto);
        assert_eq!((k.h1, k.h2), (Free, Vector));
    }

    #[test]
    fn selected_key_sets_both_handles() {
        for mode in [Vector, Align, Free] {
            let mut k = key(Auto, Auto, 0, S, 0);
            k.set_type(mode);
            assert_eq!((k.h1, k.h2), (mode, mode));
        }
    }

    #[test]
    fn single_handle_updates_opposite() {
        // Left handle to Vector: opposite Auto becomes Free.
        let mut k = key(Auto, Auto, S, 0, 0);
        k.set_type(Vector);
        assert_eq!((k.h1, k.h2), (Vector, Free));
        // Right handle to Align with opposite Vector keeps Vector.
        let mut k = key(Vector, Free, 0, 0, S);
        k.set_type(Align);
        assert_eq!((k.h1, k.h2), (Vector, Align));
        // Left to Align: opposite Auto follows.
        let mut k = key(Free, Auto, S, 0, 0);
        k.set_type(Align);
        assert_eq!((k.h1, k.h2), (Align, Align));
        // Left to Free: opposite Align becomes Free (Align <> Free not allowed).
        let mut k = key(Auto, Align, S, 0, 0);
        k.set_type(Free);
        assert_eq!((k.h1, k.h2), (Free, Free));
    }

    #[test]
    fn opposite_rules() {
        let mut k = key(AlignDoubleSide, Free, 0, 0, 0);
        k.update_from_opposite(Side::Left);
        assert_eq!(k.h2, AlignDoubleSide);
        let mut k = key(Free, Align, 0, 0, 0);
        k.update_from_opposite(Side::Right);
        assert_eq!(k.h1, Align, "Align source overwrites a non-Vector opposite");
    }

    #[test]
    fn curve_changes_only_with_selection() {
        let mut keys = [key(Auto, Auto, 0, 0, 0), key(Auto, Auto, 0, S, 0)];
        assert!(set_handle_type(&mut keys, Vector));
        assert_eq!((keys[0].h1, keys[0].h2), (Auto, Auto), "unselected key unchanged");
        assert_eq!((keys[1].h1, keys[1].h2), (Vector, Vector));
        let mut none = [key(Auto, Auto, 0, 0, 0)];
        assert!(!set_handle_type(&mut none, Vector));
        assert_eq!(none[0].h1, Auto);
    }
}
