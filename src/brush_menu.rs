//! The brush popup of the sculpt tool header: every brush with its key, the active one marked.
//!
//! It looks like the other popup menus (`add_menu.rs`: `wcol_menu_back` panel, `UI_UNIT_Y` rows, `inner_sel`
//! under the cursor) and opens with its top left corner under the brush dropdown. Blender shows brush icons and
//! the brush asset shelf here; this one is a plain list. A click outside only closes it.

use crate::area::{fill_rect, Rcti};
use crate::font;
use crate::sculpt::Brush;
use crate::theme::Rgba;
use crate::userdef::UserDef;

const OUTLINE: Rgba = Rgba::hex(0x242424ff);
const INNER: Rgba = Rgba::hex(0x181818ff);
const INNER_SEL: Rgba = Rgba::hex(0x4772b3ff);
const TITLE_TEXT: Rgba = Rgba::hex(0x999999ff);
const ITEM_TEXT: Rgba = Rgba::hex(0xddddddff);
const ITEM_TEXT_SEL: Rgba = Rgba::hex(0xffffffff);
const KEY_TEXT: Rgba = Rgba::hex(0x888888ff);
const ACTIVE_MARK: Rgba = Rgba::hex(0x4772b3ff);
const TITLE: &str = "Brush";

pub struct Layout {
    pub panel: Rcti,
    pub title: Rcti,
    pub items: Vec<(Rcti, Brush)>,
}

/// The panel for a menu opened at window position `pos` (y up) in a `win_w` x `win_h` window.
pub fn layout(user: &UserDef, pos: [i32; 2], win_w: i32, win_h: i32) -> Layout {
    let unit = user.widget_unit;
    let margin = (unit / 5).max(2);
    let gap = 2 * unit;
    let widest = Brush::ALL
        .iter()
        .map(|b| font::label_width(user, b.label()) + gap + font::string_width(user, b.key_label()))
        .max()
        .unwrap_or(0);
    let width = (widest + 2 * margin).max(8 * unit);
    let height = (Brush::ALL.len() as i32 + 1) * unit + 2 * margin;
    let mut xmin = pos[0];
    let mut ymax = pos[1];
    if xmin + width - 1 > win_w - 1 {
        xmin = win_w - width;
    }
    if ymax - height + 1 < 0 {
        ymax = height - 1;
    }
    if ymax > win_h - 1 {
        ymax = win_h - 1;
    }
    xmin = xmin.max(0);
    let panel = Rcti { xmin, xmax: xmin + width - 1, ymin: ymax - height + 1, ymax };
    let row = |i: i32| Rcti {
        xmin: panel.xmin + margin,
        xmax: panel.xmax - margin,
        ymax: panel.ymax - margin - i * unit,
        ymin: panel.ymax - margin - (i + 1) * unit + 1,
    };
    let items = Brush::ALL.iter().enumerate().map(|(i, &b)| (row(i as i32 + 1), b)).collect();
    Layout { panel, title: row(0), items }
}

pub fn item_at(layout: &Layout, cursor: [i32; 2]) -> Option<Brush> {
    layout.items.iter().find(|(r, _)| r.isect_pt_v(cursor)).map(|&(_, b)| b)
}

/// Draws the menu; `active` is the current brush, `cursor` the window position.
pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, layout: &Layout, active: Brush, cursor: [i32; 2]) {
    let b = user.pixelsize.max(1);
    fill_rect(buf, width, height, layout.panel, OUTLINE.to_xrgb());
    let inner = Rcti { xmin: layout.panel.xmin + b, xmax: layout.panel.xmax - b, ymin: layout.panel.ymin + b, ymax: layout.panel.ymax - b };
    fill_rect(buf, width, height, inner, INNER.to_xrgb());
    font::draw_label(buf, width, height, user, layout.title, TITLE, TITLE_TEXT);
    let hover = item_at(layout, cursor);
    for &(rect, brush) in &layout.items {
        let selected = hover == Some(brush);
        if selected {
            fill_rect(buf, width, height, rect, INNER_SEL.to_xrgb());
        } else if brush == active {
            /* The active brush has a thin bar at the left edge. */
            fill_rect(buf, width, height, Rcti { xmax: rect.xmin + 2 * b - 1, ..rect }, ACTIVE_MARK.to_xrgb());
        }
        let text = if selected || brush == active { ITEM_TEXT_SEL } else { ITEM_TEXT };
        font::draw_label(buf, width, height, user, rect, brush.label(), text);
        let key = brush.key_label();
        let kw = font::string_width(user, key);
        let key_color = if selected { ITEM_TEXT_SEL } else { KEY_TEXT };
        font::draw_label_pad(buf, width, height, user, rect, key, key_color, rect.size_x() + 1 - kw - (6.0 * user.scale_factor) as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserDef {
        UserDef::from_window_scale(1.0)
    }

    #[test]
    fn lists_every_brush_in_order_inside_the_panel() {
        let l = layout(&user(), [300, 500], 1000, 800);
        assert_eq!((l.panel.xmin, l.panel.ymax), (300, 500));
        let brushes: Vec<_> = l.items.iter().map(|&(_, b)| b).collect();
        assert_eq!(brushes, Brush::ALL);
        assert!(l.items.iter().all(|(r, _)| l.panel.isect_pt(r.xmin, r.ymin) && l.panel.isect_pt(r.xmax, r.ymax)));
        assert!(l.items.windows(2).all(|w| w[0].0.ymin == w[1].0.ymax + 1));
        assert!(l.title.ymin > l.items[0].0.ymax);
    }

    #[test]
    fn moves_back_into_the_window() {
        let l = layout(&user(), [995, 5], 1000, 800);
        assert_eq!(l.panel.xmax, 999);
        assert_eq!(l.panel.ymin, 0);
    }

    #[test]
    fn items_are_found_under_the_cursor() {
        let l = layout(&user(), [300, 500], 1000, 800);
        for (r, b) in &l.items {
            assert_eq!(item_at(&l, [r.cent_x(), r.cent_y()]), Some(*b));
        }
        assert_eq!(item_at(&l, [l.title.cent_x(), l.title.cent_y()]), None);
        assert_eq!(item_at(&l, [0, 0]), None);
    }

    #[test]
    fn the_hovered_row_is_highlighted() {
        if font::string_width(&user(), "A") == 0 {
            return;
        }
        let u = user();
        let (w, h) = (1000usize, 800usize);
        let l = layout(&u, [300, 500], w as i32, h as i32);
        let mut buf = vec![0u32; w * h];
        let (rect, brush) = l.items[3];
        draw(&mut buf, w, h, &u, &l, Brush::Draw, [rect.cent_x(), rect.cent_y()]);
        let idx = |x: i32, y: i32| (h - 1 - y as usize) * w + x as usize;
        assert_eq!(buf[idx(rect.xmax - 1, rect.cent_y())], INNER_SEL.to_xrgb(), "{brush:?} row is highlighted");
        let (other, _) = l.items[6];
        assert_eq!(buf[idx(other.xmax - 1, other.cent_y())], INNER.to_xrgb());
    }

    /* `cargo test dump_brush_menu -- --ignored` writes /tmp/brush_menu.ppm for a look at the popup. */
    #[test]
    #[ignore]
    fn dump_brush_menu() {
        let u = UserDef::from_window_scale(2.0);
        let (w, h) = (420usize, 700usize);
        let l = layout(&u, [20, 690], w as i32, h as i32);
        let mut buf = vec![0x0028_2828u32; w * h];
        let r = l.items[2].0;
        draw(&mut buf, w, h, &u, &l, Brush::Draw, [r.cent_x(), r.cent_y()]);
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        for p in &buf {
            out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
        }
        std::fs::write("/tmp/brush_menu.ppm", out).unwrap();
    }
}
