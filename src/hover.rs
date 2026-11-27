//! Which widget is under the cursor (`UI_HOVER`) and the colours it takes.
//!
//! Ported from Blender (editors/interface/interface_widgets.cc):
//!  - `widget_state()` / `widget_active_color()` (:3286, :3248): regular, tool, toggle, text and tab widgets
//!  - `widget_pulldownbut()` (:4971): a header menu draws a box in `wcol_pulldown.inner_sel` only while hovered
//!
//! The draw functions register their widget rects each frame, so a cursor move knows when to redraw.

use std::cell::{Cell, RefCell};

use crate::area::Rcti;
use crate::theme::Rgba;
use crate::theme_widgets::WCOL_PULLDOWN;
use crate::userdef::UserDef;
use crate::widget_colors::WidgetColors;
use crate::widget_draw::draw_round_box;
use crate::widget_hover::widget_active_color;

thread_local! {
    static CURSOR: Cell<Option<[i32; 2]>> = const { Cell::new(None) };
    static RECTS: RefCell<Vec<Rcti>> = const { RefCell::new(Vec::new()) };
}

/// Starts a redraw; `cursor` is `None` while a popup covers the widgets.
pub fn begin_frame(cursor: Option<[i32; 2]>) {
    CURSOR.set(cursor);
    RECTS.with_borrow_mut(Vec::clear);
}

/// Registers the widget `rect` and tells whether the cursor is over it.
pub fn over(rect: Rcti) -> bool {
    RECTS.with_borrow_mut(|r| r.push(rect));
    CURSOR.get().is_some_and(|c| rect.isect_pt_v(c))
}

/// Is the cursor over `rect`? Does not register a widget.
pub fn inside(rect: Rcti) -> bool {
    CURSOR.get().is_some_and(|c| rect.isect_pt_v(c))
}

/// Does moving the cursor from `old` to `new` change the hovered widget?
pub fn changed(old: [i32; 2], new: [i32; 2]) -> bool {
    RECTS.with_borrow(|r| {
        let at = |p| r.iter().position(|rect| rect.isect_pt_v(p));
        at(old) != at(new)
    })
}

fn col(c: Rgba) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

fn rgba(c: [u8; 4]) -> Rgba {
    Rgba { r: c[0], g: c[1], b: c[2], a: c[3] }
}

/// `(inner, text)` of a widget at `rect`, brightened by `widget_active_color()` while hovered.
pub fn colors(rect: Rcti, inner: Rgba, text: Rgba) -> (Rgba, Rgba) {
    if !over(rect) {
        return (inner, text);
    }
    let mut w = WidgetColors { inner: col(inner), text: col(text), ..Default::default() };
    widget_active_color(&mut w);
    (rgba(w.inner), rgba(w.text))
}

/// A header menu button: the hover box and the label colour.
pub fn pulldown(buf: &mut [u32], width: usize, height: usize, user: &UserDef, rect: Rcti) -> Rgba {
    if !over(rect) {
        return rgba(WCOL_PULLDOWN.text);
    }
    let none = Rgba::hex(0);
    draw_round_box(buf, width, height, user, rect, rgba(WCOL_PULLDOWN.inner_sel), none, WCOL_PULLDOWN.roundness);
    rgba(WCOL_PULLDOWN.text_sel)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: Rcti = Rcti { xmin: 10, xmax: 29, ymin: 10, ymax: 29 };

    #[test]
    fn only_the_hovered_widget_brightens() {
        let (inner, text) = (Rgba::hex(0x545454ff), Rgba::hex(0xe6e6e6ff));
        begin_frame(None);
        assert_eq!(colors(BOX, inner, text), (inner, text));
        begin_frame(Some([15, 15]));
        assert_eq!(colors(BOX, inner, text), (Rgba::hex(0x656565ff), Rgba::hex(0xffffffff)));
        begin_frame(Some([40, 15]));
        assert_eq!(colors(BOX, inner, text), (inner, text));
    }

    #[test]
    fn redraw_is_needed_when_crossing_a_widget_edge() {
        begin_frame(None);
        over(BOX);
        assert!(changed([0, 0], [15, 15]));
        assert!(!changed([15, 15], [16, 16]));
        assert!(!changed([0, 0], [1, 1]));
    }
}
