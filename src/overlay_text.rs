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
//! Beyond Blender: the object name is the real active object (Blender shows only `Collection` when there
//! is none) and the modifier stack of that object is listed under it, one line per modifier (`>` marks the active one, `+` the others), because
//! the modifiers have no properties editor yet.
//!
//! Deviations: the outline shadow of the text is not drawn, the grid unit line of `draw_grid_unit_name()`
//! (axis views only) ignores `View3D::grid != 1`, and the collection name is always `Collection`. Locked rotation, camera flips and local view are not
//! modelled.

use crate::area::Rcti;
use crate::font;
use crate::theme;
use crate::userdef::UserDef;
use crate::overlay_grid::grid_unit_name;
use crate::scene::Scene;
use crate::view3d::{RegionView3D, View, View3D};

/// What the info text shows about the active object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectInfo {
    pub name: Option<String>,
    /// One line per modifier, the Subdivision Surface level last.
    pub modifiers: Vec<String>,
    /// The index of the active modifier in `modifiers`, marked with `>` instead of `+`.
    pub active: usize,
}

impl ObjectInfo {
    pub fn of(scene: &Scene) -> ObjectInfo {
        match scene.active_object() {
            None => ObjectInfo::default(),
            Some(o) => {
                let mut modifiers: Vec<String> = o.modifiers.iter().map(|m| m.summary()).collect();
                if o.subsurf > 0 {
                    modifiers.push(format!("Subdivision Surface {}", o.subsurf));
                }
                let active = o.active_modifier.min(o.modifiers.len().saturating_sub(1));
                ObjectInfo { name: Some(o.name.clone()), modifiers, active }
            }
        }
    }
}

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
fn lines(v3d: &View3D, rv3d: &RegionView3D, frame: i32, object: &ObjectInfo) -> Vec<String> {
    let mut lines = vec![viewport_name(rv3d)];
    /* `draw_grid_unit_name()` draws the grid unit below the viewport name. */
    lines.extend(grid_unit_name(v3d, rv3d).map(str::to_string));
    lines.push(match &object.name {
        Some(name) => format!("Collection | {name}"),
        None => "Collection".to_string(),
    });
    lines.push(frame.to_string());
    lines.extend(object.modifiers.iter().enumerate().map(|(i, m)| format!("{} {m}", if i == object.active { '>' } else { '+' })));
    lines
}

/// Draws the info text into the main region `region` (inclusive window rect, y up).
pub fn draw(buf: &mut [u32], width: usize, height: usize, region: Rcti, user: &UserDef, v3d: &View3D, rv3d: &RegionView3D, frame: i32, object: &ObjectInfo) {
    let line_height = font::UI_DEFAULT_TEXT_POINTS * user.scale_factor * 1.6;
    let x = region.xmin + (0.5 * user.widget_unit as f32) as i32;
    let mut y = region.ymax - (0.1 * user.widget_unit as f32) as i32;
    for line in lines(v3d, rv3d, frame, object) {
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
        let cube = ObjectInfo { name: Some("Cube".to_string()), modifiers: vec![], active: 0 };
        assert_eq!(lines(&View3D::default(), &rv3d, 1, &cube), ["User Perspective", "Collection | Cube", "1"]);
    }

    #[test]
    fn no_active_object_and_modifier_lines() {
        let rv3d = RegionView3D::default();
        let none = lines(&View3D::default(), &rv3d, 1, &ObjectInfo::default());
        assert_eq!(none, ["User Perspective", "Collection", "1"]);
        let info = ObjectInfo { name: Some("Torus".to_string()), modifiers: vec!["Mirror X".to_string(), "Subdivision Surface 2".to_string()], active: 0 };
        let l = lines(&View3D::default(), &rv3d, 1, &info);
        assert_eq!(l[1], "Collection | Torus");
        assert_eq!(&l[3..], ["> Mirror X", "+ Subdivision Surface 2"]);
    }

    #[test]
    fn info_follows_the_scene() {
        let mut scene = Scene::startup();
        assert_eq!(ObjectInfo::of(&scene).name.as_deref(), Some("Cube"));
        scene.toggle_mirror(0);
        scene.toggle_bevel();
        scene.set_subsurf(2);
        let info = ObjectInfo::of(&scene);
        assert_eq!(info.modifiers, ["Mirror X", "Bevel 0.1 m", "Subdivision Surface 2"]);
        /* The Bevel was added last, so it is the active one; selecting moves the marker. */
        assert_eq!(info.active, 1);
        scene.select_modifier(-1);
        assert_eq!(ObjectInfo::of(&scene).active, 0);
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

    /* Top view, 1920x1000: the grid unit line (see `grid_unit_name()`) follows the view name. */
    #[test]
    fn grid_unit_line_in_axis_views() {
        let v3d = View3D::default();
        let mut rv3d = RegionView3D::default();
        rv3d.view_axis(View::Top);
        rv3d.update(&v3d, 1920, 1000);
        let l = lines(&v3d, &rv3d, 1, &ObjectInfo::default());
        assert_eq!(l.len(), 4);
        assert_eq!(l[1], "10 Centimeters");
    }
}
