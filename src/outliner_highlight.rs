//! Row highlights of the outliner, ported from `outliner_draw_highlights()`
//! (`source/blender/editors/space_outliner/outliner_draw.cc:3586`, :3666).
//! Colours: `.space_outliner.selected_highlight` / `.active` (userdef_default_theme.c:613, :609),
//! hover `{1, 1, 1, 0.13}` (outliner_draw.cc:3672).

use crate::area::Rcti;
use crate::scene::{RowKind, Scene};
use crate::theme::Rgba;
use crate::userdef::UserDef;

/// `OL_Y_OFFSET` (outliner_intern.hh:200).
const OL_Y_OFFSET: i32 = 2;
/// `TH_SELECT_HIGHLIGHT` of the outliner (`selected_highlight`).
const SELECT_HIGHLIGHT: Rgba = Rgba::hex(0x1d314dff);
/// `TH_SELECT_ACTIVE` of the outliner (`.active`).
const SELECT_ACTIVE: Rgba = Rgba::hex(0x334d80ff);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `TSE_ACTIVE | TSE_SELECTED`: `col_active` fill and a `TH_SELECT_ACTIVE` +40 outline.
    Active,
    /// `TSE_SELECTED`: `col_selection` fill.
    Selected,
    /// `TSE_HIGHLIGHTED`: white at alpha 0.13.
    Hover,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Highlight {
    pub rect: Rcti,
    pub kind: Kind,
}

/// The rounded boxes of `outliner_draw_highlights()` for the window `region` (inclusive rect, y up):
/// row `i` spans `starty .. starty + UI_UNIT_Y` with `starty = -OL_Y_OFFSET - UI_UNIT_Y * (i + 1)`
/// below the region top, inset by `padding_x = 3 * UI_SCALE_FAC` on both sides and `ufac` on top and bottom.
pub fn highlights(user: &UserDef, region: Rcti, scene: &Scene, hover_row: Option<usize>) -> Vec<Highlight> {
    let unit = user.widget_unit;
    let ufac = (unit as f32 / 20.0).round() as i32;
    let padding_x = (3.0 * user.scale_factor) as i32;
    let rh = region.size_y() + 1;
    let mut out = Vec::new();
    for (i, row) in scene.outliner_rows().iter().enumerate() {
        let starty = -OL_Y_OFFSET - unit * (i as i32 + 1);
        if starty < -rh {
            break;
        }
        let rect = Rcti {
            xmin: region.xmin + padding_x,
            xmax: region.xmax + 1 - padding_x - 1,
            ymin: region.ymin + rh + starty + ufac,
            ymax: region.ymin + rh + starty + unit - ufac - 1,
        };
        let selected = match row.kind {
            RowKind::Object(id) => scene.index_of(id).map(|k| &scene.objects[k]).filter(|o| o.selected && !o.hidden).map(|_| scene.active == Some(id)),
            _ => None,
        };
        match selected {
            Some(true) => out.push(Highlight { rect, kind: Kind::Active }),
            Some(false) => out.push(Highlight { rect, kind: Kind::Selected }),
            None => {}
        }
        if hover_row == Some(i) {
            out.push(Highlight { rect, kind: Kind::Hover });
        }
    }
    out
}

/// `UI_GetColorPtrShade3ubv(TH_SELECT_ACTIVE, 40)`: +40 per channel, clamped.
fn active_outline() -> Rgba {
    let s = |c: u8| c.saturating_add(40);
    Rgba { r: s(SELECT_ACTIVE.r), g: s(SELECT_ACTIVE.g), b: s(SELECT_ACTIVE.b), a: 255 }
}

fn blend(buf: &mut [u32], width: usize, height: usize, x: i32, y: i32, c: Rgba, alpha: f32) {
    if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
        return;
    }
    let p = &mut buf[(height - 1 - y as usize) * width + x as usize];
    let mix = |d: u32, s: u8| ((1.0 - alpha) * d as f32 + alpha * s as f32).round() as u32;
    *p = (mix((*p >> 16) & 0xff, c.r) << 16) | (mix((*p >> 8) & 0xff, c.g) << 8) | mix(*p & 0xff, c.b);
}

/// Distance of pixel centre `(x, y)` outside the rounded box `r` with corner radius `radius` (<= 0 inside).
fn corner_dist(r: Rcti, radius: f32, x: i32, y: i32) -> f32 {
    let (cx, cy) = ((r.xmin + r.xmax + 1) as f32 / 2.0, (r.ymin + r.ymax + 1) as f32 / 2.0);
    let (hx, hy) = ((r.xmax - r.xmin + 1) as f32 / 2.0, (r.ymax - r.ymin + 1) as f32 / 2.0);
    let (qx, qy) = ((x as f32 + 0.5 - cx).abs() - (hx - radius), (y as f32 + 0.5 - cy).abs() - (hy - radius));
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius
}

/// Draws the boxes of `highlights()` with the radius `UI_UNIT_Y / 8`.
pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti, list: &[Highlight]) {
    let radius = user.widget_unit as f32 / 8.0;
    let line = user.pixelsize.max(1) as f32;
    for h in list {
        let Some(vis) = region.isect(&h.rect) else { continue };
        for y in vis.ymin..=vis.ymax {
            for x in vis.xmin..=vis.xmax {
                let d = corner_dist(h.rect, radius, x, y);
                if d > 0.0 {
                    continue;
                }
                match h.kind {
                    Kind::Active if d > -line => blend(buf, width, height, x, y, active_outline(), 1.0),
                    Kind::Active => blend(buf, width, height, x, y, SELECT_ACTIVE, 1.0),
                    Kind::Selected => blend(buf, width, height, x, y, SELECT_HIGHLIGHT, 1.0),
                    Kind::Hover => blend(buf, width, height, x, y, Rgba::hex(0xffffffff), 0.13),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserDef {
        UserDef::from_window_scale(1.0)
    }

    #[test]
    fn selected_active_and_hover_rows_of_the_startup_scene() {
        let mut s = Scene::startup();
        for o in &mut s.objects {
            o.selected = false;
        }
        s.objects[0].selected = true;
        let id = s.objects[0].id;
        s.active = Some(id);
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 162 };
        let list = highlights(&user(), region, &s, Some(3));
        let kinds: Vec<Kind> = list.iter().map(|h| h.kind).collect();
        assert_eq!(kinds.iter().filter(|k| **k == Kind::Active).count(), 1);
        assert_eq!(kinds.iter().filter(|k| **k == Kind::Hover).count(), 1);
        /* Row 3 (Cube): view y -2 - 80 .. -62, inset 1 px; x inset 3 px, region width 200. */
        let hover = list.iter().find(|h| h.kind == Kind::Hover).unwrap();
        assert_eq!(hover.rect, Rcti { xmin: 3, xmax: 196, ymin: 163 - 82 + 1, ymax: 163 - 82 + 20 - 2 });
    }

    #[test]
    fn nothing_selected_means_no_highlight() {
        let mut s = Scene::startup();
        for o in &mut s.objects {
            o.selected = false;
        }
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 162 };
        assert!(highlights(&user(), region, &s, None).is_empty());
    }

    #[test]
    fn active_fill_outline_and_rounded_corner_pixels() {
        let region = Rcti { xmin: 0, xmax: 99, ymin: 0, ymax: 59 };
        let h = Highlight { rect: Rcti { xmin: 3, xmax: 96, ymin: 20, ymax: 37 }, kind: Kind::Active };
        let (w, hh) = (100usize, 60usize);
        let mut buf = vec![0u32; w * hh];
        draw(&mut buf, w, hh, &user(), region, &[h]);
        let at = |x: usize, y: usize| buf[(hh - 1 - y) * w + x];
        assert_eq!(at(50, 28), 0x334d80);
        assert_eq!(at(50, 20), 0x5b75a8);
        assert_eq!(at(3, 20), 0, "the corner is cut by the radius");
        assert_eq!(at(1, 28), 0);
    }

    #[test]
    fn hover_blends_white_at_13_percent() {
        let region = Rcti { xmin: 0, xmax: 99, ymin: 0, ymax: 59 };
        let h = Highlight { rect: Rcti { xmin: 3, xmax: 96, ymin: 20, ymax: 37 }, kind: Kind::Hover };
        let mut buf = vec![0x282828u32; 100 * 60];
        draw(&mut buf, 100, 60, &user(), region, &[h]);
        /* 0x28 + (0xff - 0x28) * 0.13 = 40 + 28.47 = 68 */
        assert_eq!(buf[(59 - 28) * 100 + 50], 0x444444);
    }
}
