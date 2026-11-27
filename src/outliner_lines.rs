//! The hierarchy lines of the outliner (`outliner_draw_hierarchy_lines_recursive`, `outliner_draw_hierarchy_line`,
//! outliner_draw.cc:3414-3545) as plain segments, so the tree is tested without a window. A solid vertical line runs
//! under each expanded layer collection and a dashed one (`dash_width` 8, `udash_factor` 0.5) under each expanded
//! object that has object children. Blender draws no horizontal ticks. Not modelled: the collection colour tags and
//! the half alpha of faded (hidden) elements, which the Rust tree has no data for.

use crate::area::Rcti;
use crate::scene::{OutlinerRow, RowKind};
use crate::theme::{self, Rgba};
use crate::userdef::UserDef;
use crate::widget_draw::blend_px;

/// `OL_Y_OFFSET`.
const OL_Y_OFFSET: i32 = 2;
/// `dash_width` of the line shader; half of it is drawn (`udash_factor` 0.5).
const DASH_WIDTH: i32 = 8;

/// One vertical line, in view coordinates (x right from the region, y up from the top of the tree, so `<= 0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HLine {
    pub x: i32,
    /// First vertex (`y2 + line_padding` in Blender's naming is the lower one; the line is drawn top to bottom).
    pub top: i32,
    pub bottom: i32,
    pub dashed: bool,
}

/// `get_color_blend_3ubv(TH_BACK, TH_TEXT, 0.4)` with alpha 255.
pub fn line_color() -> Rgba {
    let (back, text) = (theme::SPACE_OUTLINER_BACK, theme::SPACE_OUTLINER_TEXT);
    let mix = |a: u8, b: u8| ((1.0f32 - 0.4) * a as f32 + 0.4 * b as f32).floor() as u8;
    Rgba { r: mix(back.r, text.r), g: mix(back.g, text.g), b: mix(back.b, text.b), a: 255 }
}

/// The lines of the tree `rows` (depth first, `level` = nesting), in the order Blender draws them: a parent's line
/// after those of its subtree.
pub fn hierarchy_lines(user: &UserDef, rows: &[OutlinerRow]) -> Vec<HLine> {
    let unit = user.widget_unit;
    let startx = unit / 2 - (user.pixelsize + 1) / 2;
    let pad = unit / 4;
    let mut out = Vec::new();
    let mut starty = -OL_Y_OFFSET;
    walk(rows, 0, rows.len(), &mut starty, unit, startx, pad, &mut out);
    out
}

/// `rows[from..to]` are siblings and their subtrees; `starty` is the running row top as in Blender.
#[allow(clippy::too_many_arguments)]
fn walk(rows: &[OutlinerRow], from: usize, to: usize, starty: &mut i32, unit: i32, startx: i32, pad: i32, out: &mut Vec<HLine>) {
    let mut i = from;
    while i < to {
        let level = rows[i].level;
        /* The subtree ends at the next row that is not deeper. */
        let mut end = i + 1;
        while end < to && rows[end].level > level {
            end += 1;
        }
        *starty -= unit;
        let has_subtree = end > i + 1;
        let mut line = None;
        if has_subtree {
            let child_level = rows[i + 1].level;
            let draws = match rows[i].kind {
                RowKind::ActiveCollection | RowKind::Collection => Some(false),
                RowKind::Object(_) => rows[i + 1..end].iter().any(|r| r.level == child_level && matches!(r.kind, RowKind::Object(_))).then_some(true),
                RowKind::SceneCollection => None,
            };
            if let Some(dashed) = draws {
                line = Some((*starty, dashed));
            }
            walk(rows, i + 1, end, starty, unit, startx, pad, out);
        }
        if let Some((y, dashed)) = line {
            out.push(HLine { x: startx + level * unit, top: y - pad, bottom: *starty + pad, dashed });
        }
        i = end;
    }
}

/// Draws `lines` into the window `region` (y up), clipped to it; the dash pattern starts at the top end.
pub fn draw(buf: &mut [u32], width: usize, height: usize, region: Rcti, lines: &[HLine]) {
    let rh = region.size_y() + 1;
    let color = line_color();
    for l in lines {
        let x = region.xmin + l.x;
        if x < region.xmin || x > region.xmax || x < 0 || x as usize >= width {
            continue;
        }
        for (n, vy) in (l.bottom..l.top).rev().enumerate() {
            if l.dashed && (n as i32) % DASH_WIDTH >= DASH_WIDTH / 2 {
                continue;
            }
            let y = region.ymin + rh + vy;
            if y < region.ymin || y > region.ymax || y < 0 || y as usize >= height {
                continue;
            }
            let px = &mut buf[(height - 1 - y as usize) * width + x as usize];
            *px = blend_px(*px, color, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    fn rows(scene: &Scene) -> Vec<OutlinerRow> {
        scene.outliner_rows()
    }

    #[test]
    fn the_line_colour_is_forty_percent_text_over_the_background() {
        assert_eq!(line_color(), Rgba { r: 102, g: 102, b: 102, a: 255 });
    }

    #[test]
    fn the_startup_collection_gets_one_solid_line_down_its_objects() {
        for scale in [1.0, 2.0] {
            let user = UserDef::from_window_scale(scale);
            let unit = user.widget_unit;
            let r = rows(&Scene::startup());
            let lines = hierarchy_lines(&user, &r);
            let n = r.len() as i32;
            assert_eq!(lines.len(), 1, "the scene collection and the childless objects have none");
            /* The Collection row is row 1; its children are rows 2..n. */
            let l = lines[0];
            assert_eq!(l.x, unit / 2 - (user.pixelsize + 1) / 2 + unit);
            assert_eq!((l.top, l.bottom), (-2 - 2 * unit - unit / 4, -2 - n * unit + unit / 4));
            assert!(!l.dashed);
        }
    }

    #[test]
    fn a_parent_object_gets_a_dashed_line_and_comes_after_its_children() {
        let user = UserDef::from_window_scale(1.0);
        let unit = user.widget_unit;
        let row = |level, kind| OutlinerRow { level, name: String::new(), kind };
        let r = [
            row(0, RowKind::SceneCollection),
            row(1, RowKind::Collection),
            row(2, RowKind::Object(1)),
            row(3, RowKind::Object(2)),
            row(2, RowKind::Object(3)),
        ];
        let lines = hierarchy_lines(&user, &r);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].dashed && !lines[1].dashed, "the nested parent is drawn first");
        assert_eq!(lines[0].x, lines[1].x + unit);
        assert_eq!((lines[0].top, lines[0].bottom), (-2 - 3 * unit - unit / 4, -2 - 4 * unit + unit / 4));
        /* An object with only non-object rows below it has no line. */
        let data = [row(0, RowKind::Object(1)), row(1, RowKind::Collection)];
        assert!(hierarchy_lines(&user, &data).is_empty());
    }

    #[test]
    fn dashes_draw_four_pixels_then_skip_four() {
        let (w, h) = (20usize, 60usize);
        let mut buf = vec![0u32; w * h];
        let region = Rcti { xmin: 0, xmax: 19, ymin: 0, ymax: 59 };
        draw(&mut buf, w, h, region, &[HLine { x: 5, top: -2, bottom: -18, dashed: true }]);
        let lit: Vec<usize> = (0..h).filter(|&row| buf[row * w + 5] != 0).collect();
        /* View y -3 is the first pixel under the top vertex: window y 57, buffer row 2. */
        assert_eq!(lit, vec![2, 3, 4, 5, 10, 11, 12, 13]);
    }
}
