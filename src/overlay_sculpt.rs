//! Overlay engine "Sculpts" (draw/engines/overlay/overlay_sculpt.hh): when the sculpt mask / face set / curves cage
//! overlays run, their opacities, whether a mesh has anything to show for its PBVH type, and the curves
//! "everything selected" shortcut. Deviation: drawing and attribute lookups are the caller's job; the caller
//! passes plain booleans for the attributes present.

#![allow(dead_code)]

pub const OB_MODE_SCULPT: u32 = 1 << 1;
pub const OB_MODE_SCULPT_CURVES: u32 = 1 << 12;
pub const V3D_OVERLAY_SCULPT_SHOW_MASK: u32 = 1 << 14;
pub const V3D_OVERLAY_SCULPT_SHOW_FACE_SETS: u32 = 1 << 15;
pub const V3D_OVERLAY_SCULPT_CURVES_CAGE: u32 = 1 << 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Show {
    pub mask: bool,
    pub face_set: bool,
    pub curves_cage: bool,
}

/// `State::show_sculpt_mask/face_sets/curves_cage()`.
pub fn show_flags(overlay_flag: u32) -> Show {
    Show {
        mask: overlay_flag & V3D_OVERLAY_SCULPT_SHOW_MASK != 0,
        face_set: overlay_flag & V3D_OVERLAY_SCULPT_SHOW_FACE_SETS != 0,
        curves_cage: overlay_flag & V3D_OVERLAY_SCULPT_CURVES_CAGE != 0,
    }
}

/// `enabled_`: 3D view, not wire (X-ray opacity 0), not selection / depth-only, sculpt or curves sculpt mode,
/// and at least one overlay shown.
pub fn enabled(is_space_v3d: bool, xray_opacity: f32, is_selection: bool, is_depth_only: bool, object_mode: u32, show: Show) -> bool {
    let is_wire = xray_opacity == 0.0;
    is_space_v3d
        && !is_wire
        && !is_selection
        && !is_depth_only
        && (object_mode == OB_MODE_SCULPT_CURVES || object_mode == OB_MODE_SCULPT)
        && (show.curves_cage || show.face_set || show.mask)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opacities {
    pub curve_cage: f32,
    pub face_set: f32,
    pub mask: f32,
}

/// Opacities pushed to the passes: the overlay setting when shown, else 0.
pub fn opacities(show: Show, cage_opacity: f32, face_sets_opacity: f32, mask_opacity: f32) -> Opacities {
    Opacities {
        curve_cage: if show.curves_cage { cage_opacity } else { 0.0 },
        face_set: if show.face_set { face_sets_opacity } else { 0.0 },
        mask: if show.mask { mask_opacity } else { 0.0 },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PbvhType {
    /// `has_face_set_or_mask_attr`: the mesh has `.sculpt_face_set` or `.sculpt_mask`.
    Mesh { has_face_set_or_mask_attr: bool },
    /// Multires: subdiv CCG masks non-empty, or the base mesh has `.sculpt_face_set`.
    Grids { has_ccg_masks: bool, has_face_set_attr: bool },
    /// BMesh layers `.sculpt_face_set` (face data) or `.sculpt_mask` (vertex data).
    BMesh { has_face_set_layer: bool, has_mask_layer: bool },
}

/// `mesh_sync()` early-outs: false when there is nothing to display (the object is skipped).
pub fn mesh_has_overlay_data(show: Show, pbvh: Option<PbvhType>) -> bool {
    if !show.face_set && !show.mask {
        return false;
    }
    match pbvh {
        None => false,
        Some(PbvhType::Mesh { has_face_set_or_mask_attr }) => has_face_set_or_mask_attr,
        Some(PbvhType::Grids { has_ccg_masks, has_face_set_attr }) => has_ccg_masks || has_face_set_attr,
        Some(PbvhType::BMesh { has_face_set_layer, has_mask_layer }) => has_face_set_layer || has_mask_layer,
    }
}

/// Sculpt batch features requested from the PBVH: `(face_set, mask)`.
pub fn batch_features(show: Show) -> (bool, bool) {
    (show.face_set, show.mask)
}

/// `everything_selected()` for curves: the `.selection` attribute is a single value `true`
/// (a missing attribute defaults to true).
pub fn everything_selected(selection_single_value: Option<bool>) -> bool {
    selection_single_value == Some(true)
}

/// Curves mask draw condition: mask shown and not everything selected.
pub fn draw_curves_mask(show_mask: bool, selection_single_value: Option<bool>) -> bool {
    show_mask && !everything_selected(selection_single_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Show = Show { mask: true, face_set: true, curves_cage: true };

    #[test]
    fn show_flags_from_overlay_bits() {
        assert_eq!(show_flags(0), Show { mask: false, face_set: false, curves_cage: false });
        assert_eq!(show_flags(V3D_OVERLAY_SCULPT_SHOW_MASK | V3D_OVERLAY_SCULPT_CURVES_CAGE), Show { mask: true, face_set: false, curves_cage: true });
    }

    #[test]
    fn enabled_only_in_sculpt_modes_with_an_overlay_on() {
        assert!(enabled(true, 0.5, false, false, OB_MODE_SCULPT, ALL));
        assert!(enabled(true, 0.5, false, false, OB_MODE_SCULPT_CURVES, ALL));
        assert!(!enabled(true, 0.5, false, false, 1, ALL)); // edit mode
        assert!(!enabled(true, 0.0, false, false, OB_MODE_SCULPT, ALL)); // wire
        assert!(!enabled(true, 0.5, true, false, OB_MODE_SCULPT, ALL));
        assert!(!enabled(true, 0.5, false, true, OB_MODE_SCULPT, ALL));
        assert!(!enabled(false, 0.5, false, false, OB_MODE_SCULPT, ALL));
        assert!(!enabled(true, 0.5, false, false, OB_MODE_SCULPT, Show { mask: false, face_set: false, curves_cage: false }));
    }

    #[test]
    fn opacities_propagate_or_zero() {
        let o = opacities(Show { mask: true, face_set: false, curves_cage: true }, 0.7, 0.8, 0.9);
        assert_eq!(o, Opacities { curve_cage: 0.7, face_set: 0.0, mask: 0.9 });
    }

    #[test]
    fn mesh_data_checks_per_pbvh_type() {
        let s = ALL;
        assert!(!mesh_has_overlay_data(Show { mask: false, face_set: false, curves_cage: true }, Some(PbvhType::Mesh { has_face_set_or_mask_attr: true })));
        assert!(!mesh_has_overlay_data(s, None));
        assert!(mesh_has_overlay_data(s, Some(PbvhType::Mesh { has_face_set_or_mask_attr: true })));
        assert!(!mesh_has_overlay_data(s, Some(PbvhType::Mesh { has_face_set_or_mask_attr: false })));
        assert!(mesh_has_overlay_data(s, Some(PbvhType::Grids { has_ccg_masks: true, has_face_set_attr: false })));
        assert!(!mesh_has_overlay_data(s, Some(PbvhType::Grids { has_ccg_masks: false, has_face_set_attr: false })));
        assert!(mesh_has_overlay_data(s, Some(PbvhType::BMesh { has_face_set_layer: false, has_mask_layer: true })));
        assert!(!mesh_has_overlay_data(s, Some(PbvhType::BMesh { has_face_set_layer: false, has_mask_layer: false })));
    }

    #[test]
    fn curves_everything_selected_skips_mask() {
        assert!(everything_selected(Some(true)));
        assert!(!everything_selected(Some(false)) && !everything_selected(None));
        assert!(!draw_curves_mask(true, Some(true)));
        assert!(draw_curves_mask(true, None));
        assert!(!draw_curves_mask(false, Some(false)));
        assert_eq!(batch_features(ALL), (true, true));
    }
}
