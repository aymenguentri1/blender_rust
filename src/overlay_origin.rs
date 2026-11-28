//! Overlay engine "Origins" (draw/engines/overlay/overlay_origin.hh): when object origin points are drawn and
//! which theme colour each gets (active, selected, or deselected with "Origins (All)"), with the library variant
//! for linked/multi-user data. Deviation: returns the colour role; the theme lookup and buffers are the caller's.

#![allow(dead_code)]

pub const OB_MODE_SCULPT: u32 = 1 << 1;
pub const OB_MODE_VERTEX_PAINT: u32 = 1 << 2;
pub const OB_MODE_WEIGHT_PAINT: u32 = 1 << 3;
pub const OB_MODE_TEXTURE_PAINT: u32 = 1 << 4;
pub const OB_MODE_PAINT_GREASE_PENCIL: u32 = 1 << 8;
pub const OB_MODE_SCULPT_GREASE_PENCIL: u32 = 1 << 9;
pub const OB_MODE_WEIGHT_GREASE_PENCIL: u32 = 1 << 10;
pub const OB_MODE_VERTEX_GREASE_PENCIL: u32 = 1 << 11;
pub const OB_MODE_SCULPT_CURVES: u32 = 1 << 12;

/// `OB_MODE_ALL_PAINT`: every mode where the brush system is used.
pub const OB_MODE_ALL_PAINT: u32 = OB_MODE_SCULPT
    | OB_MODE_VERTEX_PAINT
    | OB_MODE_WEIGHT_PAINT
    | OB_MODE_TEXTURE_PAINT
    | OB_MODE_PAINT_GREASE_PENCIL
    | OB_MODE_SCULPT_GREASE_PENCIL
    | OB_MODE_WEIGHT_GREASE_PENCIL
    | OB_MODE_VERTEX_GREASE_PENCIL
    | OB_MODE_SCULPT_CURVES;

pub const V3D_OVERLAY_HIDE_OBJECT_ORIGINS: u32 = 1 << 10;
/// `V3D_DRAW_CENTERS` (View3D.flag): "Origins (All)".
pub const V3D_DRAW_CENTERS: u32 = 1 << 15;
pub const BASE_SELECTED: u32 = 1 << 0;

/// `State::show_object_origins()`.
pub fn show_object_origins(overlay_flag: u32) -> bool {
    overlay_flag & V3D_OVERLAY_HIDE_OBJECT_ORIGINS == 0
}

/// `enabled_` of `begin_sync()`: 3D view, not a paint mode, origins not hidden.
pub fn enabled(is_space_v3d: bool, object_mode: u32, overlay_flag: u32) -> bool {
    let is_paint_mode = object_mode & OB_MODE_ALL_PAINT != 0;
    is_space_v3d && !is_paint_mode && show_object_origins(overlay_flag)
}

/// `is_library`: more than one real user or linked from a library file.
pub fn is_library(real_users: i32, is_linked: bool) -> bool {
    real_users > 1 || is_linked
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginColor {
    Active,
    LibrarySelect,
    ObjectSelect,
    Library,
    Deselect,
}

/// Colour role of an object's origin point, `None` when it is not drawn. `from_dupli_or_set` objects are skipped.
pub fn origin_color(from_dupli_or_set: bool, is_active: bool, base_flag: u32, v3d_flag: u32, is_library: bool) -> Option<OriginColor> {
    if from_dupli_or_set {
        return None;
    }
    if is_active {
        Some(OriginColor::Active)
    } else if base_flag & BASE_SELECTED != 0 {
        Some(if is_library { OriginColor::LibrarySelect } else { OriginColor::ObjectSelect })
    } else if v3d_flag & V3D_DRAW_CENTERS != 0 {
        Some(if is_library { OriginColor::Library } else { OriginColor::Deselect })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_rules() {
        assert!(enabled(true, 0, 0));
        assert!(!enabled(false, 0, 0));
        assert!(!enabled(true, 0, V3D_OVERLAY_HIDE_OBJECT_ORIGINS));
        assert!(!enabled(true, OB_MODE_SCULPT, 0));
        assert!(!enabled(true, OB_MODE_SCULPT_CURVES, 0));
        assert!(enabled(true, 1, 0)); // edit mode still shows origins
    }

    #[test]
    fn active_wins_then_selected_then_all_origins() {
        assert_eq!(origin_color(false, true, BASE_SELECTED, V3D_DRAW_CENTERS, true), Some(OriginColor::Active));
        assert_eq!(origin_color(false, false, BASE_SELECTED, 0, false), Some(OriginColor::ObjectSelect));
        assert_eq!(origin_color(false, false, BASE_SELECTED, 0, true), Some(OriginColor::LibrarySelect));
        assert_eq!(origin_color(false, false, 0, V3D_DRAW_CENTERS, false), Some(OriginColor::Deselect));
        assert_eq!(origin_color(false, false, 0, V3D_DRAW_CENTERS, true), Some(OriginColor::Library));
    }

    #[test]
    fn unselected_without_all_origins_and_dupli_are_hidden() {
        assert_eq!(origin_color(false, false, 0, 0, false), None);
        assert_eq!(origin_color(true, true, BASE_SELECTED, V3D_DRAW_CENTERS, false), None);
    }

    #[test]
    fn library_needs_multiple_users_or_link() {
        assert!(!is_library(1, false));
        assert!(is_library(2, false));
        assert!(is_library(1, true));
    }
}
