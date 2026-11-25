//! The info text in the top left of the 3D viewport.
//!
//! Ported from Blender:
//!  - `view3d_draw_region_info()` text block   (editors/space_view3d/view3d_draw.cc:1602-1645): the start
//!    is `xmin + 0.5 * U.widget_unit`, `ymax - 0.1 * U.widget_unit`, each line moves down by
//!    `VIEW3D_OVERLAY_LINEHEIGHT` (`widget.points * UI_SCALE_FAC * 1.6`) and is drawn on that baseline
//!  - `view3d_get_name()`, `draw_viewport_name()` (view3d_draw.cc:1040-1220): view name, axis roll,
//!    ` (Clipped)`
//!  - `draw_selected_name()`                    (view3d_draw.cc:1243-1370): `collection | object` in
//!    object mode
//!  - `draw_frame_info()`                       (view3d_draw.cc:1416-1458): the current frame
//!  - `ED_view3d_text_colors_get()`             (view3d_utils.cc:91): `TH_TEXT_HI`
//!
//! Deviations: the outline shadow of the text is not drawn, the grid unit line (axis views only) is not
//! drawn, and the active collection and object names are those of the startup scene (`Collection`,
//! `Cube`) because there is no scene data yet. Locked rotation, camera flips and local view are not
//! modelled.

use crate::area::Rcti;
use crate::font;
use crate::theme;
use crate::userdef::UserDef;
use crate::view3d::{RegionView3D, View};

/// `view3d_get_name()` for an unlocked view without a camera object.
fn view_name(rv3d: &RegionView3D) -> &'static str {
    match (rv3d.view, rv3d.ortho) {
        (View::Front, true) => "Front Orthographic",
        (View::Front, false) => "Front Perspective",
        (View::Back, true) => "Back Orthographic",
        (View::Back, false) => "Back Perspective",
        (View::Top, true) => "Top Orthographic",
        (View::Top, false) => "Top Perspective",
        (View::Bottom, true) => "Bottom Orthographic",
        (View::Bottom, false) => "Bottom Perspective",
        (View::Right, true) => "Right Orthographic",
        (View::Right, false) => "Right Perspective",
        (View::Left, true) => "Left Orthographic",
        (View::Left, false) => "Left Perspective",
        (View::User, true) => "User Orthographic",
        (View::User, false) => "User Perspective",
    }
}

/// `draw_viewport_name()`: the view name with the axis roll and the clipping marker.
fn viewport_name(rv3d: &RegionView3D) -> String {
    let mut name = view_name(rv3d).to_string();
    if rv3d.view != View::User {
        match rv3d.view_axis_roll & 3 {
            1 => name.push_str(" 90\u{b0}"),
            2 => name.push_str(" 180\u{b0}"),
            3 => name.push_str(" -90\u{b0}"),
            _ => {}
        }
    }
    if rv3d.clip.is_some() {
        name.push_str(" (Clipped)");
    }
    name
}

/// The lines of the info text, top to bottom.
fn lines(rv3d: &RegionView3D, frame: i32) -> [String; 3] {
    [viewport_name(rv3d), "Collection | Cube".to_string(), frame.to_string()]
}

/// Draws the info text into the main region `region` (inclusive window rect, y up).
pub fn draw(buf: &mut [u32], width: usize, height: usize, region: Rcti, user: &UserDef, rv3d: &RegionView3D, frame: i32) {
    let line_height = font::UI_DEFAULT_TEXT_POINTS * user.scale_factor * 1.6;
    let x = region.xmin + (0.5 * user.widget_unit as f32) as i32;
    let mut y = region.ymax - (0.1 * user.widget_unit as f32) as i32;
    for line in lines(rv3d, frame) {
        y = (y as f32 - line_height) as i32;
        font::draw_text(buf, width, height, user, x, y, &line, theme::SPACE_VIEW3D_TEXT_HI);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_view_lines() {
        let rv3d = RegionView3D::default();
        assert_eq!(lines(&rv3d, 1), ["User Perspective", "Collection | Cube", "1"]);
    }

    #[test]
    fn axis_view_name_has_roll_and_clipping() {
        let mut rv3d = RegionView3D::default();
        rv3d.view = View::Top;
        rv3d.ortho = true;
        rv3d.view_axis_roll = 1;
        rv3d.clip = Some([[0.0; 4]; 4]);
        assert_eq!(viewport_name(&rv3d), "Top Orthographic 90\u{b0} (Clipped)");
    }
}
