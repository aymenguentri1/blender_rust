//! Background of the outliner's window region.
//!
//! Ported from Blender:
//!  - `outliner_back()`        (editors/space_outliner/outliner_draw.cc:3775-3811): every second row is drawn
//!    with `TH_BACK` blended towards `TH_ROW_ALTERNATE` by the alpha of the latter
//!  - `OL_Y_OFFSET`            (editors/space_outliner/outliner_intern.hh:200)
//!
//! The view is the initial one: `View2D::tot.ymax` is 0 and, with `V2D_ALIGN_NO_POS_Y`
//! (space_outliner.cc:75), `cur.ymax` is 0 too (the tree is not scrolled), so `cur.ymin` is minus the region
//! height. The names of the startup scene's tree rows are drawn by `draw_tree_names()`
//! (`outliner_draw_tree()` / `outliner_draw_tree_element()`, outliner_draw.cc:3162-3774); icons, expand
//! arrows, highlights and the restriction columns are not drawn.

use crate::area::{fill_rect, top_header_rect, Rcti};
use crate::font;
use crate::theme;
use crate::userdef::UserDef;

/// `OL_Y_OFFSET`.
const OL_Y_OFFSET: i32 = 2;

/// The outliner's window region: what is left of `totrct` below the (non-overlapping) header.
pub fn window_rect(user: &UserDef, totrct: Rcti) -> Option<Rcti> {
    let header = top_header_rect(user, totrct)?;
    let rect = Rcti { ymax: header.ymin - 1, ..totrct };
    if rect.is_empty() { None } else { Some(rect) }
}

/// `outliner_back()` for the window `region` (inclusive Blender rect, y up) of the `0x00RRGGBB` buffer.
pub fn draw_row_stripes(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti) {
    let rh = region.size_y() + 1;
    let unit = user.widget_unit;
    let ystart = -OL_Y_OFFSET;
    /* `int(floor(ystart - cur.ymin + 2 * UI_UNIT_Y)) / (2 * UI_UNIT_Y)`, with `cur.ymin = -rh`. */
    let tot = (ystart + rh + 2 * unit) / (2 * unit);

    /* `immUniformThemeColorBlend(TH_BACK, TH_ROW_ALTERNATE, alpha)`, stored as 8 bit. */
    let (back, alt) = (theme::SPACE_OUTLINER_BACK, theme::SPACE_OUTLINER_ROW_ALTERNATE);
    let fac = alt.a as f32 / 255.0;
    let mix = |a: u8, b: u8| ((1.0 - fac) * a as f32 + fac * b as f32).round() as u32;
    let color = (mix(back.r, alt.r) << 16) | (mix(back.g, alt.g) << 8) | mix(back.b, alt.b);

    for k in 1..=tot {
        /* View2D y of the stripe: `y1 .. y1 + UI_UNIT_Y`, with the region top at y = 0. */
        let y1 = ystart - 2 * unit * k;
        let y2 = y1 + unit;
        let stripe = Rcti { ymin: region.ymin + rh + y1, ymax: region.ymin + rh - 1 + y2, ..region };
        if let Some(visible) = region.isect(&stripe) {
            fill_rect(buf, width, height, visible, color);
        }
    }
}

/// The tree of the startup scene in the view layer display mode, top to bottom: the nesting level, the
/// name (`TreeElement::name`) and the text colour that `outliner_draw_tree_element()` picks:
/// `TH_TEXT`, `TH_TEXT_HI` for the active layer collection (`tree_element_type_active_state_get()`) and
/// `TH_ACTIVE_OBJECT` for the selected, active object. Collections and objects are sorted by name.
const TREE: [(i32, &str, theme::Rgba); 6] = [
    (0, "Scene Collection", theme::SPACE_OUTLINER_TEXT),
    (1, "Collection", theme::SPACE_OUTLINER_TEXT_HI),
    (2, "Camera", theme::SPACE_OUTLINER_TEXT),
    (2, "Cube", theme::SPACE_OUTLINER_ACTIVE_OBJECT),
    (2, "Light", theme::SPACE_OUTLINER_TEXT),
    (1, "XGen Guides", theme::SPACE_OUTLINER_TEXT),
];

/// The names of `outliner_draw_tree()` for the window `region` (inclusive Blender rect, y up) of the
/// `0x00RRGGBB` buffer: the first row has `starty = tot.ymax - UI_UNIT_Y - OL_Y_OFFSET` and each row is
/// `UI_UNIT_Y` lower; the element is at `startx = level * UI_UNIT_X`, then `offsx` skips the expand arrow
/// slot (`UI_UNIT_X`) and the icon slot (`UI_UNIT_X + 4 * ufac`), and the text baseline is `starty + 5 * ufac`
/// with `ufac = UI_UNIT_X / 20`. The icons that fill those slots are not drawn, and a row is skipped when it
/// is not entirely inside the region (Blender clips with the region scissor instead).
pub fn draw_tree_names(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti) {
    let rh = region.size_y() + 1;
    let unit = user.widget_unit;
    let ufac = unit as f32 / 20.0;
    let offsx = (2.0 * unit as f32 + 4.0 * ufac) as i32;

    for (i, &(level, name, color)) in TREE.iter().enumerate() {
        let starty = -OL_Y_OFFSET - unit * (i as i32 + 1);
        if starty < -rh {
            break;
        }
        let x = region.xmin + level * unit + offsx;
        /* View2D y of the baseline to window y: the region top is view y 0. */
        let baseline = region.ymin + rh + (starty as f32 + 5.0 * ufac).round() as i32;
        font::draw_text(buf, width, height, user, x, baseline, name, color);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /* 1x, 100x100 region: tot = (-2 + 100 + 40) / 40 = 3; the first stripe is the second row, 22..42 px below
     * the top, i.e. region y 58..=77; the next one is y 18..=37. 0x28 blended 4/255 towards 0xff is 43. */
    #[test]
    fn stripes_on_every_second_row() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (100usize, 100usize);
        let bg = theme::SPACE_OUTLINER_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let region = Rcti { xmin: 0, xmax: 99, ymin: 0, ymax: 99 };
        draw_row_stripes(&mut buf, w, h, &user, region);

        let at = |y: usize| buf[(h - 1 - y) * w + 50];
        assert_eq!(at(58), 0x002b_2b2b);
        assert_eq!(at(77), 0x002b_2b2b);
        assert_eq!(at(78), bg);
        assert_eq!(at(40), bg);
        assert_eq!(at(18), 0x002b_2b2b);
        assert_eq!(at(37), 0x002b_2b2b);
        assert_eq!(at(0), bg);
    }

    #[test]
    fn window_below_header() {
        let user = UserDef::from_window_scale(1.0);
        let totrct = Rcti { xmin: 1579, xmax: 1917, ymin: 864, ymax: 1052 };
        assert_eq!(window_rect(&user, totrct), Some(Rcti { xmin: 1579, xmax: 1917, ymin: 864, ymax: 1026 }));
    }

    /* With the font file present, in a 200x163 region at 1x (rh = 163): the Cube row (index 3) has
     * starty = -2 - 80 = -82, so its baseline is y = 163 - 82 + 5 = 86 and its text starts at x = 40 + 44 =
     * 84 in the active object colour (red well above blue); the rows end above y = 40 (XGen Guides baseline 46). */
    #[test]
    fn tree_names_are_drawn() {
        let user = UserDef::from_window_scale(1.0);
        if font::string_width(&user, "Cube") == 0 {
            return;
        }
        let (w, h) = (200usize, 163usize);
        let bg = theme::SPACE_OUTLINER_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 162 };
        draw_tree_names(&mut buf, w, h, &user, region);

        let at = |x: usize, y: usize| buf[(h - 1 - y) * w + x];
        let orange = (84..110).any(|x| (84..98).any(|y| {
            let p = at(x, y);
            ((p >> 16) & 0xff) > (p & 0xff) + 60
        }));
        assert!(orange);
        assert!((0..w).all(|x| (0..40).all(|y| at(x, y) == bg)));
    }
}
