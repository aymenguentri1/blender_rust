//! The Add > Mesh popup menu of Shift + A over the viewport.
//!
//! Modelled on Blender's popup menus (`ui_popup_menu_*`, `wcol_menu_back` / `wcol_menu_item` of
//! userdef_default_theme.c:152-171): a dark panel with a one pixel outline, a title row, and one row of
//! `UI_UNIT_Y` per entry that is highlighted (`inner_sel`) under the cursor. The panel opens with its top left
//! corner at the cursor and is moved back into the window when it would stick out. Deviations: the corners are
//! square, there is no Mesh submenu level (the nine mesh primitives are listed directly), and a click outside
//! only closes the menu (Blender also closes it when the cursor moves far away).

use crate::area::{fill_rect, Rcti};
use crate::font;
use crate::mesh::Primitive;
use crate::theme::Rgba;
use crate::userdef::UserDef;

const OUTLINE: Rgba = Rgba::hex(0x242424ff);
const INNER: Rgba = Rgba::hex(0x181818ff);
const INNER_SEL: Rgba = Rgba::hex(0x4772b3ff);
const TITLE_TEXT: Rgba = Rgba::hex(0x999999ff);
const ITEM_TEXT: Rgba = Rgba::hex(0xddddddff);
const ITEM_TEXT_SEL: Rgba = Rgba::hex(0xffffffff);
const TITLE: &str = "Add";

/// An entry of the menu. Blender nests the meshes in a Mesh submenu; here all entries are in one list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddItem {
    Mesh(Primitive),
    Empty,
    Camera,
    Light,
}

impl AddItem {
    /// The nine mesh primitives, then Empty, Camera and Light (the order of `VIEW3D_MT_add`).
    pub const ALL: [AddItem; 12] = [
        AddItem::Mesh(Primitive::Plane),
        AddItem::Mesh(Primitive::Cube),
        AddItem::Mesh(Primitive::Circle),
        AddItem::Mesh(Primitive::UvSphere),
        AddItem::Mesh(Primitive::IcoSphere),
        AddItem::Mesh(Primitive::Cylinder),
        AddItem::Mesh(Primitive::Cone),
        AddItem::Mesh(Primitive::Torus),
        AddItem::Mesh(Primitive::Grid),
        AddItem::Empty,
        AddItem::Camera,
        AddItem::Light,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AddItem::Mesh(p) => p.label(),
            AddItem::Empty => "Empty",
            AddItem::Camera => "Camera",
            AddItem::Light => "Light",
        }
    }

    /// The name of the new object.
    pub fn object_name(self) -> &'static str {
        match self {
            AddItem::Mesh(p) => p.object_name(),
            other => other.label(),
        }
    }
}

pub struct Layout {
    pub panel: Rcti,
    pub title: Rcti,
    pub items: Vec<(Rcti, AddItem)>,
}

/// The panel for a menu opened at window position `pos` (y up) in a `win_w` x `win_h` window.
pub fn layout(user: &UserDef, pos: [i32; 2], win_w: i32, win_h: i32) -> Layout {
    let unit = user.widget_unit;
    let margin = (unit / 5).max(2);
    let widest = AddItem::ALL
        .iter()
        .map(|p| font::label_width(user, p.label()))
        .chain([font::label_width(user, TITLE)])
        .max()
        .unwrap_or(0);
    let width = (widest + 2 * margin).max(6 * unit);
    let rows = AddItem::ALL.len() as i32 + 1;
    let height = rows * unit + 2 * margin;

    /* Top left corner at the cursor, then back inside the window. */
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
    let items = AddItem::ALL.iter().enumerate().map(|(i, &p)| (row(i as i32 + 1), p)).collect();
    Layout { panel, title: row(0), items }
}

/// The entry under the window position `cursor`.
pub fn item_at(layout: &Layout, cursor: [i32; 2]) -> Option<AddItem> {
    layout.items.iter().find(|(r, _)| r.isect_pt_v(cursor)).map(|&(_, p)| p)
}

/// Draws the menu into the `0x00RRGGBB` buffer of `width` x `height` pixels; `cursor` is the window position.
pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, layout: &Layout, cursor: [i32; 2]) {
    let b = user.pixelsize.max(1);
    fill_rect(buf, width, height, layout.panel, OUTLINE.to_xrgb());
    let inner = Rcti {
        xmin: layout.panel.xmin + b,
        xmax: layout.panel.xmax - b,
        ymin: layout.panel.ymin + b,
        ymax: layout.panel.ymax - b,
    };
    fill_rect(buf, width, height, inner, INNER.to_xrgb());
    font::draw_label(buf, width, height, user, layout.title, TITLE, TITLE_TEXT);
    let hover = item_at(layout, cursor);
    for &(rect, prim) in &layout.items {
        let selected = hover == Some(prim);
        if selected {
            fill_rect(buf, width, height, rect, INNER_SEL.to_xrgb());
        }
        font::draw_label(buf, width, height, user, rect, prim.label(), if selected { ITEM_TEXT_SEL } else { ITEM_TEXT });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserDef {
        UserDef::from_window_scale(1.0)
    }

    #[test]
    fn the_panel_opens_below_and_right_of_the_cursor() {
        let l = layout(&user(), [300, 500], 1000, 800);
        assert_eq!((l.panel.xmin, l.panel.ymax), (300, 500));
        assert_eq!(l.items.len(), 12);
        assert!(l.items.iter().all(|(r, _)| l.panel.isect_pt(r.xmin, r.ymin) && l.panel.isect_pt(r.xmax, r.ymax)));
    }

    /* 13 rows of 20 px plus a 4 px margin on both sides. */
    #[test]
    fn rows_are_one_unit_high_and_in_order() {
        let l = layout(&user(), [300, 500], 1000, 800);
        assert_eq!(l.panel.size_y() + 1, 13 * 20 + 8);
        for (r, _) in &l.items {
            assert_eq!(r.size_y() + 1, 20);
        }
        assert!(l.title.ymin > l.items[0].0.ymax);
        assert!(l.items.windows(2).all(|w| w[0].0.ymin == w[1].0.ymax + 1));
        let labels: Vec<_> = l.items.iter().map(|(_, p)| p.label()).collect();
        assert_eq!(
            labels,
            ["Plane", "Cube", "Circle", "UV Sphere", "Ico Sphere", "Cylinder", "Cone", "Torus", "Grid", "Empty", "Camera", "Light"]
        );
    }

    #[test]
    fn the_panel_is_moved_back_into_the_window() {
        let u = user();
        let l = layout(&u, [990, 10], 1000, 800);
        assert_eq!(l.panel.xmax, 999);
        assert_eq!(l.panel.ymin, 0);
        let l = layout(&u, [0, 5000], 1000, 800);
        assert_eq!(l.panel.ymax, 799);
        assert_eq!(l.panel.xmin, 0);
    }

    #[test]
    fn items_are_found_under_the_cursor() {
        let l = layout(&user(), [300, 500], 1000, 800);
        for (r, p) in &l.items {
            assert_eq!(item_at(&l, [r.cent_x(), (r.ymin + r.ymax) / 2]), Some(*p));
        }
        assert_eq!(item_at(&l, [l.title.cent_x(), l.title.ymin + 5]), None, "the title is not an entry");
        assert_eq!(item_at(&l, [0, 0]), None);
    }

    #[test]
    fn drawing_highlights_the_hovered_row_only() {
        let u = user();
        let (w, h) = (400usize, 300usize);
        let l = layout(&u, [50, 250], w as i32, h as i32);
        let bg = 0x0055_5555u32;
        let mut buf = vec![bg; w * h];
        let (rect, _) = l.items[3];
        let cursor = [rect.cent_x(), (rect.ymin + rect.ymax) / 2];
        draw(&mut buf, w, h, &u, &l, cursor);
        let at = |x: i32, y: i32| buf[(h as i32 - 1 - y) as usize * w + x as usize];
        assert_eq!(at(rect.xmax, rect.ymin), INNER_SEL.to_xrgb(), "hovered row");
        let other = l.items[5].0;
        assert_eq!(at(other.xmax, other.ymin), INNER.to_xrgb(), "other rows");
        assert_eq!(at(l.panel.xmin, l.panel.ymin), OUTLINE.to_xrgb(), "outline");
        assert_eq!(at(l.panel.xmax + 5, l.panel.ymin), bg, "outside the panel");
    }
}
