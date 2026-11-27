//! Drawing of the startup splash popup (`wm_block_splash_create`, wm_splash_screen.cc) with the rows of `WM_MT_splash`
//! (wm.py:3571): "New File" templates left, "Getting Started" links right, a line, then Open / Recover Last Session and
//! What's New / Donate. Geometry comes from `splash_layout`. Deviations: the splash image comes from `splash_image` (the
//! LFS object `splash.png` cannot be fetched here), rows have no icons yet, and the rows do nothing but
//! highlight (a click anywhere closes the popup, as the image button does).

use crate::area::{fill_rect, Rcti};
use crate::font;
use crate::splash_layout::{self, SplashLayout};
use crate::theme::Rgba;
use crate::userdef::UserDef;
use crate::version;

const OUTLINE: Rgba = Rgba::hex(0x242424ff);
const INNER: Rgba = Rgba::hex(0x181818ff);
const INNER_SEL: Rgba = Rgba::hex(0x4772b3ff);
const TITLE_TEXT: Rgba = Rgba::hex(0x999999ff);
const ITEM_TEXT: Rgba = Rgba::hex(0xddddddff);
const ITEM_TEXT_SEL: Rgba = Rgba::hex(0xffffffff);
const LINE: Rgba = Rgba::hex(0x00000040);

const NEW_FILE: [&str; 5] = ["General", "2D Animation", "Sculpting", "VFX", "Video Editing"];
const LINKS: [&str; 5] = ["Manual", "Support", "User Communities", "Get Involved", "Blender Website"];
const BOTTOM_LEFT: [&str; 2] = ["Open...", "Recover Last Session"];
const BOTTOM_RIGHT: [&str; 2] = ["What's New", "Donate to Blender"];
/// Rows of the menu column: title, five entries, three separators (`separator()`, `LINE`, `separator()`), two entries,
/// and the trailing two `separator()` calls.
const SPACE_ROWS: f32 = 0.5;

/// A row: the text, its rectangle, whether it is a title.
pub struct Row {
    pub rect: Rcti,
    pub text: &'static str,
    pub title: bool,
}

pub struct Splash {
    pub layout: SplashLayout,
    pub rows: Vec<Row>,
    pub line: Rcti,
}

/// Height of the menu column in pixels.
pub fn menu_height(user: &UserDef) -> i32 {
    let u = user.widget_unit;
    let sep = (SPACE_ROWS * u as f32) as i32;
    /* title + 5 rows, 3 separators, 2 rows, 2 trailing separators */
    6 * u + 3 * sep + 2 * u + 2 * sep
}

pub fn layout(user: &UserDef, win_w: i32, win_h: i32, img: [i32; 2]) -> Splash {
    let l = splash_layout::layout(user, win_w, win_h, img, menu_height(user));
    let u = user.widget_unit;
    let sep = (SPACE_ROWS * u as f32) as i32;
    let half = l.menu.size_x() / 2;
    let (lx, rx) = (l.menu.xmin, l.menu.xmin + half);
    let col = |x: i32, y_top: i32, text: &'static str, title: bool| Row {
        rect: Rcti { xmin: x, xmax: x + half - 1, ymin: y_top - u + 1, ymax: y_top },
        text,
        title,
    };
    let mut rows = vec![col(lx, l.menu.ymax, "New File", true), col(rx, l.menu.ymax, "Getting Started", true)];
    for i in 0..5 {
        let y = l.menu.ymax - u * (i as i32 + 1);
        rows.push(col(lx, y, NEW_FILE[i], false));
        rows.push(col(rx, y, LINKS[i], false));
    }
    let y_line = l.menu.ymax - 6 * u - sep;
    let line = Rcti { xmin: l.menu.xmin, xmax: l.menu.xmax, ymin: y_line - sep / 2, ymax: y_line - sep / 2 };
    let y_bottom = l.menu.ymax - 6 * u - 3 * sep;
    for i in 0..2 {
        let y = y_bottom - u * i as i32;
        rows.push(col(lx, y, BOTTOM_LEFT[i], false));
        rows.push(col(rx, y, BOTTOM_RIGHT[i], false));
    }
    Splash { layout: l, rows, line }
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, s: &Splash, cursor: [i32; 2]) {
    let b = user.pixelsize.max(1);
    let p = s.layout.popup;
    fill_rect(buf, width, height, p, OUTLINE.to_xrgb());
    fill_rect(buf, width, height, Rcti { xmin: p.xmin + b, xmax: p.xmax - b, ymin: p.ymin + b, ymax: p.ymax - b }, INNER.to_xrgb());
    crate::splash_image::draw(buf, width, height, s.layout.image);
    let white = Rgba::hex(0xffffffff);
    let text = version::version_string();
    let v = Rcti { xmin: s.layout.version.xmax - font::string_width(user, &text), ..s.layout.version };
    font::draw_label_pad(buf, width, height, user, v, &text, white, 0);
    for r in &s.rows {
        if r.title {
            font::draw_label(buf, width, height, user, r.rect, r.text, TITLE_TEXT);
            continue;
        }
        let sel = r.rect.isect_pt_v(cursor);
        if sel {
            fill_rect(buf, width, height, r.rect, INNER_SEL.to_xrgb());
        }
        font::draw_label(buf, width, height, user, r.rect, r.text, if sel { ITEM_TEXT_SEL } else { ITEM_TEXT });
    }
    crate::area::blend_rect(buf, width, height, s.line, LINE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_stay_inside_the_popup_and_the_menu() {
        let u = UserDef::from_window_scale(1.0);
        let s = layout(&u, 1920, 1080, [1000, 500]);
        assert_eq!(s.rows.len(), 16);
        for r in &s.rows {
            assert!(s.layout.popup.isect_pt(r.rect.xmin, r.rect.ymin) && s.layout.popup.isect_pt(r.rect.xmax, r.rect.ymax));
        }
        assert_eq!(s.layout.menu.size_y(), menu_height(&u));
    }

    #[test]
    fn the_popup_is_centred() {
        let u = UserDef::from_window_scale(1.0);
        let s = layout(&u, 1920, 1080, [1000, 500]);
        assert!((s.layout.popup.cent_x() - 960).abs() <= 1);
    }
}
