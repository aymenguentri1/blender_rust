//! The input status of the status bar while the cursor is over a header region.
//!
//! Ported from Blender:
//!  - `uiTemplateInputStatus()` and `uiTemplateInputStatusHeader()`
//!    (interface_template_status.cc:218-232, 267-375): over a header region the row is the
//!    middle mouse drag icon, `Pan`, the right mouse icon and `Options`
//!  - `Layout::separator()`   (interface_layout.cc:3595): a space is `int(6 * UI_SCALE_FAC * factor)` wide
//!  - `uiItem_simple()`, `text_icon_width_ex()` (interface_layout.cc:3280, 438): an icon only label is
//!    `UI_UNIT_X` wide, a text label is the text plus `ceil(UI_UNIT_X * 0.25)` (`text_pad_none`)
//!  - `Layout::row(true)`     (interface_layout.cc:5155): an aligned row has no spacing between items
//!
//! Deviations: the text
//! starts at the left edge of its label because the label margin of `interface_widgets.cc` was not
//! supplied. Only the header of the editor areas is handled: over the top bar and the status bar
//! Blender shows nothing. The workspace status text, the modal keymap status, the action zones, the
//! gaps between editors, the 3D view warnings and the cursor keymap status are not modelled.

use crate::area::{self, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::screen::ScreenLayout;
use crate::theme;
use crate::ui_layout::HeaderRow;
use crate::userdef::UserDef;

/// One item of the aligned row.
enum Item {
    /// `Layout::label("", icon)`.
    Icon(crate::icons::Icon),
    /// `Layout::separator(factor)`.
    Space(f32),
    /// `Layout::label(text, ICON_NONE)`.
    Text(&'static str),
}

/// `uiTemplateInputStatusHeader()`.
const HEADER_ITEMS: [Item; 7] = [
    Item::Icon(Icon::MouseMmbDrag),
    Item::Space(-0.2),
    Item::Text("Pan"),
    Item::Space(0.6),
    Item::Icon(Icon::MouseRmb),
    Item::Space(-0.9),
    Item::Text("Options"),
];

/// Width of `Layout::separator(factor)` outside menus.
fn space_width(user: &UserDef, factor: f32) -> i32 {
    (6.0 * user.scale_factor * factor) as i32
}

/// True when `xy` (window coordinates, y up) is over the header region of an editor area.
pub fn over_editor_header(user: &UserDef, layout: &ScreenLayout, xy: [i32; 2]) -> bool {
    layout
        .editor_areas()
        .into_iter()
        .any(|(_, totrct, _)| area::top_header_rect(user, totrct).is_some_and(|header| header.isect_pt_v(xy)))
}

/// Draws the input status into the status bar header `bar` when the cursor is over an editor header.
pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, layout: &ScreenLayout, cursor: [i32; 2], bar: Rcti) {
    if !over_editor_header(user, layout, cursor) {
        return;
    }
    let mut row = HeaderRow::new_global(user, bar.size_y() + 1);
    for item in &HEADER_ITEMS {
        match *item {
            Item::Icon(icon) => {
                let mut rect = row.place_packed(user.widget_unit);
                rect.translate(bar.xmin, bar.ymin);
                icons::draw_icon(buf, width, height, user, rect, icon, theme::SPACE_STATUSBAR_TEXT);
            }
            Item::Space(factor) => {
                row.place_packed(space_width(user, factor));
            }
            Item::Text(text) => {
                let label_w = font::string_width(user, text) + (user.widget_unit as f32 * 0.25).ceil() as i32;
                let mut rect = row.place_packed(label_w);
                rect.translate(bar.xmin, bar.ymin);
                font::draw_label_pad(buf, width, height, user, rect, text, theme::SPACE_STATUSBAR_TEXT, 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::screen_layout;

    /* 1x: `int(6 * factor)` truncates towards zero: -0.2 -> -1, 0.6 -> 3, -0.9 -> -5. */
    #[test]
    fn spaces_truncate_towards_zero() {
        let user = UserDef::from_window_scale(1.0);
        assert_eq!(space_width(&user, -0.2), -1);
        assert_eq!(space_width(&user, 0.6), 3);
        assert_eq!(space_width(&user, -0.9), -5);
    }

    /* 1x 1920x1080: the 3D viewport header is y 1027..=1052; the viewport centre and the top bar are not headers. */
    #[test]
    fn header_detection() {
        let user = UserDef::from_window_scale(1.0);
        let l = screen_layout(&user, 1920, 1080);
        assert!(over_editor_header(&user, &l, [100, 1040]));
        assert!(!over_editor_header(&user, &l, [100, 500]));
        assert!(!over_editor_header(&user, &l, [100, 1070]));
    }
}
