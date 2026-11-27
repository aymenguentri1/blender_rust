//! Horizontal layout of buttons in a header region.
//!
//! Follows `ED_region_header_layout()` (editors/screen/area.cc:4236-4300): the first button starts
//! at the header offset, buttons share one height and are vertically centred, and the layout
//! direction is horizontal (`ui::LayoutDirection::Horizontal`). Items are separated by
//! `uiStyle::buttonspacex` (interface_style.cc:90), the default spacing of a layout.
//!
//! Rects are in region coordinates (origin bottom-left, inclusive like `Rcti`). Nothing is drawn
//! here: this is layout and hit-testing only.

use crate::area::{global_header_layout_start, Rcti};
use crate::userdef::UserDef;

/// A row of items placed left to right.
pub struct HeaderRow {
    next_x: i32,
    top_y: i32,
    height: i32,
    space_x: i32,
    items: Vec<Rcti>,
}

impl HeaderRow {
    /// Starts the row of a global header (top bar, status bar) of height `winy`.
    pub fn new_global(user: &UserDef, winy: i32) -> Self {
        let (x, top_y, height) = global_header_layout_start(user, winy);
        Self { next_x: x, top_y, height, space_x: user.button_space_x, items: Vec::new() }
    }

    /// Adds an item `width` pixels wide and returns its rect.
    pub fn place(&mut self, width: i32) -> Rcti {
        let rect = Rcti {
            xmin: self.next_x,
            xmax: self.next_x + width - 1,
            ymin: self.top_y - self.height,
            ymax: self.top_y - 1,
        };
        self.next_x += width + self.space_x;
        self.items.push(rect);
        rect
    }

    /// Adds an item of `width` pixels (negative for a `Layout::separator()` with a negative factor)
    /// to an aligned row (`Layout::row(true)`, `space_ = 0`): no spacing follows it.
    pub fn place_packed(&mut self, width: i32) -> Rcti {
        let rect = Rcti {
            xmin: self.next_x,
            xmax: self.next_x + width - 1,
            ymin: self.top_y - self.height,
            ymax: self.top_y - 1,
        };
        self.next_x += width;
        self.items.push(rect);
        rect
    }

    /// Index of the item under `xy` (region coordinates), like finding the button under the cursor.
    pub fn item_at(&self, xy: [i32; 2]) -> Option<usize> {
        self.items.iter().position(|r| r.isect_pt_v(xy))
    }
}

/// Window rect of the wireframe toggle in the 3D viewport header `header` (window coordinates).
/// Deviation: `UI_HEADER_OFFSET`, the start offset of a non-global header, is not in the supplied
/// source, so the offset of global headers is used.
pub fn wire_button_rect(user: &UserDef, header: Rcti) -> Rcti {
    let mut row = HeaderRow::new_global(user, header.size_y() + 1);
    let mut rect = row.place(user.widget_unit * 3);
    rect.translate(header.xmin, header.ymin);
    rect
}

/// Window rect of the X-ray toggle, the item after the wireframe toggle in the same row.
pub fn xray_button_rect(user: &UserDef, header: Rcti) -> Rcti {
    let mut row = HeaderRow::new_global(user, header.size_y() + 1);
    row.place(user.widget_unit * 3);
    let mut rect = row.place(user.widget_unit * 3);
    rect.translate(header.xmin, header.ymin);
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    /* 1x, 26 px header: offset 4, buttons 20 px high with the top at 23, so y spans 3..=22.
     * Two 40 px items are separated by 8 px: x 4..=43 and 52..=91. */
    #[test]
    fn items_are_placed_left_to_right() {
        let user = UserDef::from_window_scale(1.0);
        let mut row = HeaderRow::new_global(&user, 26);
        let a = row.place(40);
        let b = row.place(40);
        assert_eq!(a, Rcti { xmin: 4, xmax: 43, ymin: 3, ymax: 22 });
        assert_eq!(b, Rcti { xmin: 52, xmax: 91, ymin: 3, ymax: 22 });
        assert_eq!(row.item_at([10, 10]), Some(0));
        assert_eq!(row.item_at([47, 10]), None);
        assert_eq!(row.item_at([60, 10]), Some(1));
        assert_eq!(row.item_at([60, 30]), None);
    }

    #[test]
    fn wire_button_is_inside_the_header() {
        let user = UserDef::from_window_scale(1.0);
        let header = Rcti { xmin: 0, xmax: 999, ymin: 1000, ymax: 1025 };
        let r = wire_button_rect(&user, header);
        assert_eq!(r, Rcti { xmin: 4, xmax: 63, ymin: 1003, ymax: 1022 });
        /* The X-ray toggle follows after 8 px of spacing. */
        assert_eq!(xray_button_rect(&user, header), Rcti { xmin: 72, xmax: 131, ymin: 1003, ymax: 1022 });
    }

    #[test]
    fn top_bar_logo_precedes_the_menus_and_plus_follows_the_tabs() {
        /* `TOPBAR_MT_editor_menus` (logo menu, File, ...) then `template_ID_tabs` (tabs, then the `+` button). */
        let user = UserDef::from_window_scale(1.0);
        let mut row = HeaderRow::new_global(&user, 26);
        let logo = row.place(user.widget_unit);
        let file = row.place(40);
        let tab = row.place(60);
        let plus = row.place(user.widget_unit);
        assert!(logo.xmax < file.xmin && file.xmax < tab.xmin && tab.xmax < plus.xmin);
        assert_eq!(plus.size_x() + 1, user.widget_unit);
    }
}
