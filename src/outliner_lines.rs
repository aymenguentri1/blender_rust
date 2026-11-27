//! Hierarchy lines of the outliner, ported from `outliner_draw_hierarchy_lines_recursive()` and
//! `outliner_draw_hierarchy_line()` (`source/blender/editors/space_outliner/outliner_draw.cc:3426`, :3443,
//! :3517, called at :3745). Only expanded collections with children draw a line here (the scene
//! collection is a `TSE_VIEW_COLLECTION_BASE` and objects are collapsed), so the line is solid.

use crate::area::{fill_rect, Rcti};
use crate::scene::{RowKind, Scene};
use crate::theme::{Rgba, SPACE_OUTLINER_BACK, SPACE_OUTLINER_TEXT};
use crate::userdef::UserDef;

/// `OL_Y_OFFSET` (outliner_intern.hh:200).
const OL_Y_OFFSET: i32 = 2;

/// A vertical line: window `x`, from `y_bottom` to `y_top` inclusive (y up).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub x: i32,
    pub y_bottom: i32,
    pub y_top: i32,
}

/// `ui::theme::get_color_blend_3ubv(TH_BACK, TH_TEXT, 0.4f)`.
pub fn line_color() -> Rgba {
    let mix = |a: u8, b: u8| (0.6 * a as f32 + 0.4 * b as f32) as u8;
    Rgba { r: mix(SPACE_OUTLINER_BACK.r, SPACE_OUTLINER_TEXT.r), g: mix(SPACE_OUTLINER_BACK.g, SPACE_OUTLINER_TEXT.g), b: mix(SPACE_OUTLINER_BACK.b, SPACE_OUTLINER_TEXT.b), a: 255 }
}

/// The lines for the window `region`. `startx = UI_UNIT_X / 2 - (U.pixelsize + 1) / 2`, plus `UI_UNIT_X`
/// per nesting level; `y1` is the `starty` of the collection row, `y2` the one of its last descendant, and
/// `line_padding = UI_UNIT_Y / 4` is cut off both ends.
pub fn segments(user: &UserDef, region: Rcti, scene: &Scene) -> Vec<Segment> {
    let unit = user.widget_unit;
    let rh = region.size_y() + 1;
    let startx = unit / 2 - (user.pixelsize + 1) / 2;
    let padding = (unit as f32 / 4.0) as i32;
    let rows = scene.outliner_rows();
    let starty = |i: usize| -OL_Y_OFFSET - unit * (i as i32 + 1);
    let mut out = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        /* `TSE_LAYER_COLLECTION`: the scene collection itself draws no line. */
        if !matches!(row.kind, RowKind::ActiveCollection | RowKind::Collection) {
            continue;
        }
        let last = (i + 1..rows.len()).take_while(|&k| rows[k].level > row.level).last();
        let Some(last) = last else { continue };
        out.push(Segment {
            x: region.xmin + startx + row.level * unit,
            y_bottom: region.ymin + rh + starty(last) + padding,
            y_top: region.ymin + rh + starty(i) - padding,
        });
    }
    out
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti, list: &[Segment]) {
    let c = line_color().to_xrgb();
    for s in list {
        let r = Rcti { xmin: s.x, xmax: s.x + user.pixelsize.max(1) - 1, ymin: s.y_bottom, ymax: s.y_top };
        if let Some(v) = region.isect(&r) {
            fill_rect(buf, width, height, v, c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_scene_has_one_line_beside_the_collection_children() {
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 162 };
        /* Collection is row 1 (starty -42), Light row 4 (starty -102); x = 10 - 1 + 20; padding 5. */
        assert_eq!(segments(&user, region, &Scene::startup()), [Segment { x: 29, y_bottom: 163 - 102 + 5, y_top: 163 - 42 - 5 }]);
    }

    #[test]
    fn two_x_scale_doubles_the_geometry() {
        let user = UserDef::from_window_scale(2.0);
        let region = Rcti { xmin: 0, xmax: 399, ymin: 0, ymax: 325 };
        let s = segments(&user, region, &Scene::startup());
        let u = user.widget_unit;
        assert_eq!(s[0].x, u / 2 - (user.pixelsize + 1) / 2 + u);
        assert_eq!(s[0].y_top - s[0].y_bottom, 3 * u - 2 * (u / 4));
    }

    #[test]
    fn colour_is_the_40_percent_blend_of_back_and_text() {
        assert_eq!(line_color(), Rgba { r: 102, g: 102, b: 102, a: 255 });
    }

    #[test]
    fn the_line_is_drawn_in_the_column() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (200usize, 163usize);
        let mut buf = vec![0u32; w * h];
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 162 };
        draw(&mut buf, w, h, &user, region, &segments(&user, region, &Scene::startup()));
        assert_eq!(buf[(h - 1 - 80) * w + 29], 0x666666);
        assert_eq!(buf[(h - 1 - 80) * w + 28], 0);
    }
}
