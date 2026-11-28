//! Overlay engine "Facing" (draw/engines/overlay/overlay_facing.hh): when the face-orientation overlay runs,
//! its cull/depth state and which objects it draws. Deviation: state is returned as a small struct, drawing is
//! the caller's job.

#![allow(dead_code)]

/// `eDrawType` (DNA_object_enums.h).
pub const OB_WIRE: i32 = 2;
pub const OB_SOLID: i32 = 3;

pub const V3D_OVERLAY_FACE_ORIENTATION: u32 = 1 << 0;
pub const V3D_SHADING_BACKFACE_CULLING: u32 = 1 << 10;

/// `State::show_face_orientation()`.
pub fn show_face_orientation(overlay_flag: u32) -> bool {
    overlay_flag & V3D_OVERLAY_FACE_ORIENTATION != 0
}

/// `enabled_` of `begin_sync()`.
pub fn enabled(has_v3d: bool, overlay_flag: u32, xray_enabled: bool, is_selection: bool, is_depth_only_drawing: bool) -> bool {
    has_v3d && show_face_orientation(overlay_flag) && !xray_enabled && !is_selection && !is_depth_only_drawing
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepthTest {
    /// Solid mode: keeps transparent textures correct (#128113).
    Equal,
    /// Other modes, e.g. EEVEE (#114000).
    LessEqual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FacingState {
    pub cull_back: bool,
    pub depth: DepthTest,
}

/// Cull and depth state from the viewport shading type and flags.
pub fn pass_state(shading_type: i32, shading_flag: u32) -> FacingState {
    let solid = shading_type == OB_SOLID;
    FacingState {
        cull_back: solid && shading_flag & V3D_SHADING_BACKFACE_CULLING != 0,
        depth: if solid { DepthTest::Equal } else { DepthTest::LessEqual },
    }
}

/// `draw_facing` of `object_sync()`: surface drawn (`dt >= WIRE` and renderable or wire) and `dt >= SOLID`.
pub fn draw_facing(dt: i32, renderable: bool) -> bool {
    let draw_surface = dt >= OB_WIRE && (renderable || dt == OB_WIRE);
    draw_surface && dt >= OB_SOLID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_needs_flag_and_no_xray_selection_or_depth_only() {
        let f = V3D_OVERLAY_FACE_ORIENTATION;
        assert!(enabled(true, f, false, false, false));
        assert!(!enabled(false, f, false, false, false));
        assert!(!enabled(true, 0, false, false, false));
        assert!(!enabled(true, f, true, false, false));
        assert!(!enabled(true, f, false, true, false));
        assert!(!enabled(true, f, false, false, true));
    }

    #[test]
    fn cull_only_in_solid_with_backface_flag() {
        assert!(pass_state(OB_SOLID, V3D_SHADING_BACKFACE_CULLING).cull_back);
        assert!(!pass_state(OB_SOLID, 0).cull_back);
        assert!(!pass_state(4, V3D_SHADING_BACKFACE_CULLING).cull_back);
    }

    #[test]
    fn depth_test_equal_in_solid_else_less_equal() {
        assert_eq!(pass_state(OB_SOLID, 0).depth, DepthTest::Equal);
        assert_eq!(pass_state(4, 0).depth, DepthTest::LessEqual);
    }

    #[test]
    fn objects_need_solid_draw_type_and_renderable() {
        assert!(draw_facing(3, true));
        assert!(!draw_facing(3, false));
        assert!(!draw_facing(2, true)); // wire has no facing
        assert!(!draw_facing(1, true));
    }
}
