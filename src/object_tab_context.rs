//! The two rows above the panels of the Object tab: the context path (`buttons_panel_context_draw`,
//! buttons_context.cc:1305: object icon and name on the left, the pin on the right) and the active object ID row
//! (`OBJECT_PT_context_object`, properties_object.py:21: `template_ID` = browse button plus name field).
//! Deviation, as in properties_tabs.rs: the offsets come from measuring a Blender 5.1.1 Object tab at 1x (region
//! left edge x=28, panels 8 px in, rows 20 px high, 36 px between the two row tops, first panel header 30 px under
//! the ID row), not from the panel layout code. 

use crate::area::Rcti;
use crate::font;
use crate::icons::{self, Icon};
use crate::theme;
use crate::userdef::UserDef;
use crate::widget_draw::draw_round_box;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextRows {
    /// The object icon of the path row.
    pub path_icon: Rcti,
    /// The pin button at the right end of the path row.
    pub pin: Rcti,
    /// The ID browse button of the `template_ID` row.
    pub browse: Rcti,
    /// The name field of the `template_ID` row.
    pub name: Rcti,
    /// The top row of the first panel header (`panel_frame` header_rect.ymax).
    pub panels_top: i32,
}

/// `UI_PANEL_MARGIN_X` of the panels (`panel_frame::margins`).
fn side(user: &UserDef) -> i32 {
    (user.widget_unit as f32 * 0.4) as i32
}

/// The rows inside the properties window region `window` (y up).
pub fn layout(user: &UserDef, window: Rcti) -> Option<ContextRows> {
    let (s, unit) = (user.scale_factor, user.widget_unit);
    let off = |px: f32| (px * s) as i32;
    let (x0, x1) = (window.xmin + side(user), window.xmax - side(user));
    if x1 - x0 < unit * 8 {
        return None;
    }
    let row = |top: i32| (top - unit + 1, top);
    let (p0, p1) = row(window.ymax - off(9.0));
    let (n0, n1) = row(window.ymax - off(45.0));
    let browse_w = (unit as f32 * 1.6) as i32;
    Some(ContextRows {
        path_icon: Rcti { xmin: x0 + off(1.0), xmax: x0 + off(1.0) + unit - 1, ymin: p0, ymax: p1 },
        pin: Rcti { xmin: x1 - unit + 1, xmax: x1, ymin: p0, ymax: p1 },
        browse: Rcti { xmin: x0, xmax: x0 + browse_w - 1, ymin: n0, ymax: n1 },
        name: Rcti { xmin: x0 + browse_w, xmax: x1, ymin: n0, ymax: n1 },
        panels_top: window.ymax - off(75.0),
    })
}

/// Draws only the context path row (every object-rooted tab has it, e.g. Modifiers).
pub fn draw_path(buf: &mut [u32], width: usize, height: usize, user: &UserDef, rows: &ContextRows, name: &str) {
    let text = theme::WCOL_PULLDOWN_TEXT;
    /* `buttons_panel_context_draw()`: `uiItemLDrag(name, ICON_OBJECT_DATA)` and the `BUTTONS_OT_toggle_pin` button. */
    let icon = Icon::ObjectData;
    icons::draw_icon(buf, width, height, user, rows.path_icon, icon, crate::icon_theme::theme_color(icon).unwrap_or(text));
    let label = Rcti { xmin: rows.path_icon.xmax + 1, xmax: rows.pin.xmin - 1, ..rows.path_icon };
    font::draw_label_pad(buf, width, height, user, label, name, text, 0);
    icons::draw_icon(buf, width, height, user, rows.pin, Icon::Unpinned, text);
}

/// Draws the rows of the Object tab for an object called `name`: the path row and the `OBJECT_PT_context_object` row.
pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, rows: &ContextRows, name: &str) {
    let text = theme::WCOL_PULLDOWN_TEXT;
    draw_path(buf, width, height, user, rows, name);
    /* The ID row: a square-cornered-inside pair, the browse button and the text field. */
    let none = theme::WCOL_TOOL_OUTLINE;
    draw_round_box(buf, width, height, user, rows.browse, theme::WCOL_MENU_INNER, none, theme::WCOL_TOOL_ROUNDNESS);
    icons::draw_icon(buf, width, height, user, rows.browse, Icon::ObjectData, text);
    draw_round_box(buf, width, height, user, rows.name, crate::theme::Rgba::hex(0x1d1d1dff), none, theme::WCOL_TOOL_ROUNDNESS);
    font::draw_label(buf, width, height, user, rows.name, name, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    /* The properties window of the measured screenshot: x 28..337, region top at pixel row 29 (y-up value 757). */
    const WINDOW: Rcti = Rcti { xmin: 28, xmax: 337, ymin: 0, ymax: 757 };

    /// Pixel row of a y-up value in the 787 px tall screenshot.
    fn px(y: i32) -> i32 {
        786 - y
    }

    #[test]
    fn rows_land_on_the_measured_pixels_at_1x() {
        let user = UserDef::from_window_scale(1.0);
        let r = layout(&user, WINDOW).unwrap();
        /* Path icon box 37..56 x 38..57, ID row 36..329 x 74..93. */
        assert_eq!((r.path_icon.xmin, r.path_icon.xmax), (37, 56));
        assert_eq!((px(r.path_icon.ymax), px(r.path_icon.ymin)), (38, 57));
        assert_eq!((px(r.browse.ymax), px(r.browse.ymin)), (74, 93));
        assert_eq!((r.browse.xmin, r.name.xmax), (36, 329));
        assert_eq!(r.browse.xmax + 1, r.name.xmin);
        assert_eq!(r.pin.xmax, 329);
        assert_eq!(px(r.panels_top), 104, "the Transform frame outline");
    }

    #[test]
    fn rows_scale_with_the_ui_at_2x() {
        let user = UserDef::from_window_scale(2.0);
        let win = Rcti { xmax: 28 + 2 * 309, ..WINDOW };
        let r = layout(&user, win).unwrap();
        let one = layout(&UserDef::from_window_scale(1.0), WINDOW).unwrap();
        assert_eq!(r.browse.size_y(), 2 * one.browse.size_y() + 1);
        assert_eq!(win.ymax - r.panels_top, 2 * (WINDOW.ymax - one.panels_top));
        assert_eq!(win.ymax - r.browse.ymax, 2 * (WINDOW.ymax - one.browse.ymax));
        assert!(r.path_icon.xmax < r.pin.xmin);
    }

    #[test]
    fn a_narrow_window_has_no_rows() {
        let user = UserDef::from_window_scale(1.0);
        assert_eq!(layout(&user, Rcti { xmax: 28 + 100, ..WINDOW }), None);
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;

    #[test]
    fn name_row_is_above_the_transform_panel() {
        let user = UserDef::from_window_scale(1.0);
        let window = Rcti { xmin: 28, xmax: 337, ymin: 0, ymax: 757 };
        let r = layout(&user, window).unwrap();
        let tp = crate::object_tab_view::layout(&user, window, &crate::scene::Scene::startup(), None, Default::default());
        let first = tp.widgets.iter().map(|w| w.rect.ymax).max().unwrap();
        assert!(r.browse.ymin > first, "{} above {}", r.browse.ymin, first);
        assert!(r.path_icon.ymin > r.browse.ymax);
    }
}
