//! Overlay engine "Fade" (draw/engines/overlay/overlay_fade.hh): which objects get the edit/pose fade overlay
//! (`effective_mode`, `overlay_should_fade_object`, the `draw_fade` surface test) and the fade colours' alpha.
//! Deviation: modes are plain bit flags (DNA_object_enums.h); drawing is the caller's job.

#![allow(dead_code)]

pub const OB_MODE_OBJECT: u32 = 0;
pub const OB_MODE_EDIT: u32 = 1 << 0;
pub const OB_MODE_POSE: u32 = 1 << 6;

/// `effective_mode()`: with multi-object editing (Edit mode) the shared mode bits, otherwise the object's mode
/// only when it is the active object.
pub fn effective_mode(ob_mode: u32, ob_is_active: bool, active_mode: u32) -> u32 {
    if active_mode == OB_MODE_EDIT {
        return active_mode & ob_mode;
    }
    if ob_is_active { ob_mode } else { OB_MODE_OBJECT }
}

/// `overlay_should_fade_object()`; `None` for a missing active object.
pub fn should_fade_object(ob_mode: u32, ob_is_active: bool, active_mode: Option<u32>) -> bool {
    let Some(active_mode) = active_mode else { return false };
    if active_mode == OB_MODE_OBJECT || active_mode == OB_MODE_POSE {
        return false;
    }
    active_mode != effective_mode(ob_mode, ob_is_active, active_mode)
}

pub const OB_WIRE: i32 = 2;

/// `draw_surface` of `object_sync()`: draw type at least wire and (renderable or exactly wire).
pub fn draw_surface(dt: i32, renderable: bool) -> bool {
    dt >= OB_WIRE && (renderable || dt == OB_WIRE)
}

/// `begin_sync()`: the fade pass is on for the 3D view, not in selection, when the edit-mesh fade (not X-ray) or
/// the pose geometry fade applies.
pub fn fade_enabled(is_space_v3d: bool, xray_enabled: bool, show_fade_inactive: bool, do_pose_fade_geom: bool, is_selection: bool) -> bool {
    let do_edit_mesh_fade_geom = !xray_enabled && show_fade_inactive;
    is_space_v3d && (do_edit_mesh_fade_geom || do_pose_fade_geom) && !is_selection
}

/// Alpha of the pose fade colours: `(active, other)` = `(xray_alpha_bone, powf(xray_alpha_bone, 4))`.
pub fn pose_fade_alphas(xray_alpha_bone: f32) -> (f32, f32) {
    (xray_alpha_bone, xray_alpha_bone.powf(4.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    const SCULPT: u32 = 1 << 1;

    #[test]
    fn effective_mode_shares_bits_in_edit_mode_only() {
        assert_eq!(effective_mode(OB_MODE_EDIT, false, OB_MODE_EDIT), OB_MODE_EDIT);
        assert_eq!(effective_mode(OB_MODE_OBJECT, false, OB_MODE_EDIT), OB_MODE_OBJECT);
        assert_eq!(effective_mode(SCULPT, true, SCULPT), SCULPT);
        assert_eq!(effective_mode(SCULPT, false, SCULPT), OB_MODE_OBJECT);
    }

    #[test]
    fn object_and_pose_active_modes_never_fade() {
        assert!(!should_fade_object(OB_MODE_OBJECT, false, Some(OB_MODE_OBJECT)));
        assert!(!should_fade_object(OB_MODE_OBJECT, false, Some(OB_MODE_POSE)));
        assert!(!should_fade_object(OB_MODE_EDIT, true, None));
    }

    #[test]
    fn edit_mode_fades_objects_not_in_edit_mode() {
        assert!(should_fade_object(OB_MODE_OBJECT, false, Some(OB_MODE_EDIT)));
        assert!(!should_fade_object(OB_MODE_EDIT, false, Some(OB_MODE_EDIT)));
        assert!(!should_fade_object(OB_MODE_EDIT, true, Some(OB_MODE_EDIT)));
    }

    #[test]
    fn sculpt_mode_fades_everything_but_the_active_object() {
        assert!(should_fade_object(OB_MODE_OBJECT, false, Some(SCULPT)));
        assert!(!should_fade_object(SCULPT, true, Some(SCULPT)));
    }

    #[test]
    fn surface_and_enable_rules() {
        assert!(draw_surface(2, false) && !draw_surface(1, true) && draw_surface(3, true) && !draw_surface(3, false));
        assert!(fade_enabled(true, false, true, false, false));
        assert!(!fade_enabled(true, true, true, false, false)); // xray blocks edit fade
        assert!(fade_enabled(true, true, false, true, false)); // pose fade
        assert!(!fade_enabled(true, false, true, false, true) && !fade_enabled(false, false, true, true, false));
    }

    #[test]
    fn pose_alpha_is_fourth_power_for_others() {
        let (a, o) = pose_fade_alphas(0.5);
        assert_eq!(a, 0.5);
        assert!((o - 0.0625).abs() < 1e-7);
    }
}
