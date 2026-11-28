//! What the View menu rows do (`VIEW3D_MT_view`): maps an operator row to a viewport command and runs it on the
//! region view like the keymap items do (NUMPAD_1/3/7 `view3d.view_axis`, NUMPAD_5 `view3d.view_persportho`, HOME
//! `view3d.view_all`, Shift + C `view_all` with `center`, Alt + HOME `view_center_cursor`; `view3d_edit.cc`,
//! `view3d_navigate_view_all.cc`). Rows with `align_active`, camera rows and the rest are not available.

use crate::view3d::{RegionView3D, View, View3D};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewCommand {
    Axis(View),
    PerspOrtho,
    /// `view3d.view_all`; `center` also puts the 3D cursor back at the origin and includes it in the bounds.
    FrameAll { center: bool },
    /// `view3d.view_selected`.
    FrameSelected,
    CenterCursor,
}

fn prop<'a>(props: &'a [(&str, &str)], name: &str) -> Option<&'a str> {
    props.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}

pub fn command_of(idname: &str, props: &[(&str, &str)]) -> Option<ViewCommand> {
    match idname {
        "view3d.view_axis" => {
            if prop(props, "align_active") == Some("True") {
                return None;
            }
            Some(ViewCommand::Axis(match prop(props, "type")? {
                "FRONT" => View::Front,
                "BACK" => View::Back,
                "LEFT" => View::Left,
                "RIGHT" => View::Right,
                "TOP" => View::Top,
                "BOTTOM" => View::Bottom,
                _ => return None,
            }))
        }
        "view3d.view_persportho" => Some(ViewCommand::PerspOrtho),
        "view3d.view_all" => Some(ViewCommand::FrameAll { center: prop(props, "center") == Some("True") }),
        "view3d.view_selected" if prop(props, "use_all_regions") != Some("True") => Some(ViewCommand::FrameSelected),
        "view3d.view_center_cursor" => Some(ViewCommand::CenterCursor),
        _ => None,
    }
}

/// The world the commands read: bounds of the visible objects (`all`), of the selected ones, the 3D cursor.
pub struct World {
    pub all: Option<([f32; 3], [f32; 3])>,
    pub selected: Option<([f32; 3], [f32; 3])>,
    pub cursor: [f32; 3],
}

/// Runs `cmd` on the view of a `win` = `[w, h]` region. Returns `None` when nothing changed, else whether the 3D
/// cursor was moved to the origin (`center` of `view_all`). The caller starts the smooth view.
pub fn apply(rv3d: &mut RegionView3D, v3d: &View3D, cmd: ViewCommand, win: [i32; 2], w: &World) -> Option<bool> {
    match cmd {
        ViewCommand::Axis(v) => rv3d.view_axis(v),
        ViewCommand::PerspOrtho => rv3d.toggle_persp_ortho(),
        ViewCommand::CenterCursor => rv3d.center_cursor(w.cursor),
        ViewCommand::FrameAll { center } => {
            let (mut min, mut max) = rv3d.visible_bounds_of(w.all);
            if center {
                min = min.map(|v| v.min(0.0));
                max = max.map(|v| v.max(0.0));
            }
            rv3d.view_from_minmax(v3d, win[0], win[1], min, max);
            return Some(center);
        }
        ViewCommand::FrameSelected => {
            let (min, max) = w.selected?;
            rv3d.view_from_minmax(v3d, win[0], win[1], min, max);
        }
    }
    Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_map_to_commands() {
        assert_eq!(command_of("view3d.view_axis", &[("type", "TOP")]), Some(ViewCommand::Axis(View::Top)));
        assert_eq!(command_of("view3d.view_axis", &[("align_active", "True"), ("type", "TOP")]), None);
        assert_eq!(command_of("view3d.view_all", &[("center", "False")]), Some(ViewCommand::FrameAll { center: false }));
        assert_eq!(command_of("view3d.view_all", &[("center", "True")]), Some(ViewCommand::FrameAll { center: true }));
        assert_eq!(command_of("view3d.view_selected", &[("use_all_regions", "True")]), None);
        assert_eq!(command_of("view3d.view_camera", &[]), None);
    }

    fn world() -> World {
        World { all: Some(([-1.0; 3], [1.0; 3])), selected: None, cursor: [2.0, 0.0, 0.0] }
    }

    #[test]
    fn viewpoints_set_the_axis_view_and_go_orthographic() {
        let (mut r, v) = (RegionView3D::default(), View3D::default());
        for (view, quat) in [(View::Top, [1.0, 0.0, 0.0, 0.0]), (View::Front, [0.70710678, -0.70710678, 0.0, 0.0])] {
            assert!(apply(&mut r, &v, ViewCommand::Axis(view), [800, 600], &world()).is_some());
            assert!(r.ortho);
            assert_eq!(r.view, view);
            for i in 0..4 {
                assert!((r.viewquat[i] - quat[i]).abs() < 1e-5, "{view:?} {:?}", r.viewquat);
            }
        }
    }

    #[test]
    fn persportho_toggles() {
        let (mut r, v) = (RegionView3D::default(), View3D::default());
        let before = r.ortho;
        apply(&mut r, &v, ViewCommand::PerspOrtho, [800, 600], &world());
        assert_eq!(r.ortho, !before);
    }

    #[test]
    fn frame_all_centers_the_bounds_and_view_all_center_reports_the_cursor_reset() {
        let (mut r, v) = (RegionView3D::default(), View3D::default());
        let w = World { all: Some(([2.0; 3], [4.0; 3])), ..world() };
        assert_eq!(apply(&mut r, &v, ViewCommand::FrameAll { center: false }, [800, 600], &w), Some(false));
        assert_eq!(r.ofs, [-3.0; 3]);
        assert_eq!(apply(&mut r, &v, ViewCommand::FrameAll { center: true }, [800, 600], &w), Some(true));
        assert_eq!(r.ofs, [-2.0; 3]);
    }

    #[test]
    fn frame_selected_needs_a_selection_and_center_cursor_moves_ofs() {
        let (mut r, v) = (RegionView3D::default(), View3D::default());
        assert_eq!(apply(&mut r, &v, ViewCommand::FrameSelected, [800, 600], &world()), None);
        let w = World { selected: Some(([0.0, 0.0, 0.0], [2.0, 2.0, 2.0])), ..world() };
        assert!(apply(&mut r, &v, ViewCommand::FrameSelected, [800, 600], &w).is_some());
        assert_eq!(r.ofs, [-1.0; 3]);
        apply(&mut r, &v, ViewCommand::CenterCursor, [800, 600], &world());
        assert_eq!(r.ofs, [-2.0, 0.0, 0.0]);
    }
}
