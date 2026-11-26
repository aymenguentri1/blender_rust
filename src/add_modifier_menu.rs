//! The `Add Modifier` popup of the Modifiers panel (`OBJECT_MT_modifier_add`, `properties_data_modifier.py`).
//!
//! Blender's button opens a menu with one submenu per category (Edit, Generate, Deform, Normals, Physics, ...), each
//! listing its modifiers alphabetically with their icons. Here only the two categories that have modifiers in this
//! port are offered, Generate and Deform; Edit, Normals and Physics would be empty. The look follows the other popups
//! (`modifier_menu`, `add_menu`): a dark panel with an outline, one row of `UI_UNIT_Y` per entry, the entry under the
//! cursor highlighted, an arrow at the right of a category. The submenu opens beside the hovered category and stays
//! open while the cursor is inside the menu. Deviations: square corners, no search field (Blender 4.x types to
//! search), no keyboard navigation, and a click outside only closes it.
//!
//! Layout and hit-testing are plain functions of the position, so they are tested without a window.

use crate::area::{fill_rect, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::modifier_panel::type_icon;
use crate::modifiers::Modifier;
use crate::theme::Rgba;
use crate::userdef::UserDef;

const OUTLINE: Rgba = Rgba::hex(0x242424ff);
const INNER: Rgba = Rgba::hex(0x181818ff);
const INNER_SEL: Rgba = Rgba::hex(0x4772b3ff);
const ITEM_TEXT: Rgba = Rgba::hex(0xddddddff);
const ITEM_TEXT_SEL: Rgba = Rgba::hex(0xffffffff);
const ICON_BLUE: Rgba = Rgba::hex(0x8ab4ebff);

/// The categories and their modifiers (by `Modifier::label`), in Blender's alphabetical order.
pub const CATEGORIES: [(&str, &[&str]); 2] = [
    ("Generate", &["Array", "Bevel", "Boolean", "Build", "Decimate", "Edge Split", "Mirror", "Remesh", "Screw", "Solidify", "Triangulate", "Weld", "Wireframe"]),
    ("Deform", &["Cast", "Corrective Smooth", "Displace", "Laplacian Smooth", "Shrinkwrap", "Simple Deform", "Smooth", "Wave"]),
];

/// The index of `label` in `Modifier::catalog()`, which is what `modifier_panel::Action::Add` takes.
pub fn catalog_index(label: &str) -> Option<usize> {
    Modifier::catalog().iter().position(|m| m.label() == label)
}

/// What the cursor is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// A row of the first level: opens (or keeps open) that category.
    Category(usize),
    /// An entry of the open submenu: the index in `Modifier::catalog()`.
    Item(usize),
}

pub struct Layout {
    pub root: Rcti,
    pub rows: Vec<(Rcti, usize)>,
    /// The open submenu: its panel and its entries (`catalog index`, label).
    pub sub: Option<(Rcti, Vec<(Rcti, usize, &'static str)>)>,
    pub open: Option<usize>,
}

struct Metrics {
    unit: i32,
    margin: i32,
}

fn panel_for(m: &Metrics, rows: usize, width: i32, xmin: i32, ymax: i32, win_h: i32) -> Rcti {
    let height = rows as i32 * m.unit + 2 * m.margin;
    let ymax = ymax.min(win_h - 1).max(height - 1);
    Rcti { xmin, xmax: xmin + width - 1, ymin: ymax - height + 1, ymax }
}

/// The menu opened at window position `pos` (y up, the top left corner) in a `win_w` x `win_h` window, with the
/// submenu of category `open` shown.
pub fn layout(user: &UserDef, pos: [i32; 2], win_w: i32, win_h: i32, open: Option<usize>) -> Layout {
    let m = Metrics { unit: user.widget_unit, margin: (user.widget_unit / 5).max(2) };
    let widest = CATEGORIES.iter().map(|(n, _)| font::label_width(user, n)).max().unwrap_or(0);
    let width = (widest + 2 * m.margin + 2 * m.unit).max(8 * m.unit);
    let xmin = pos[0].min(win_w - width).max(0);
    let root = panel_for(&m, CATEGORIES.len(), width, xmin, pos[1], win_h);
    let row = |panel: &Rcti, i: usize| Rcti {
        xmin: panel.xmin + m.margin,
        xmax: panel.xmax - m.margin,
        ymax: panel.ymax - m.margin - i as i32 * m.unit,
        ymin: panel.ymax - m.margin - (i as i32 + 1) * m.unit + 1,
    };
    let rows: Vec<(Rcti, usize)> = (0..CATEGORIES.len()).map(|i| (row(&root, i), i)).collect();
    let sub = open.filter(|&c| c < CATEGORIES.len()).map(|c| {
        let labels = CATEGORIES[c].1;
        let widest = labels.iter().map(|l| font::label_width(user, l)).max().unwrap_or(0);
        let sw = (widest + 2 * m.margin + 2 * m.unit).max(8 * m.unit);
        /* Beside the category row, flipped to the left when it would leave the window. */
        let right = root.xmax - m.margin;
        let sx = if right + sw - 1 > win_w - 1 { (root.xmin - sw + m.margin).max(0) } else { right };
        let panel = panel_for(&m, labels.len(), sw, sx, rows[c].0.ymax + m.margin, win_h);
        let items = labels.iter().enumerate().filter_map(|(i, l)| Some((row(&panel, i), catalog_index(l)?, *l))).collect();
        (panel, items)
    });
    Layout { root, rows, sub, open }
}

/// The row or entry under the window position `cursor`.
pub fn hit(layout: &Layout, cursor: [i32; 2]) -> Option<Hit> {
    if let Some((_, items)) = &layout.sub {
        if let Some((_, idx, _)) = items.iter().find(|(r, _, _)| r.isect_pt_v(cursor)) {
            return Some(Hit::Item(*idx));
        }
    }
    layout.rows.iter().find(|(r, _)| r.isect_pt_v(cursor)).map(|&(_, c)| Hit::Category(c))
}

/// Is the cursor on the menu at all (first level or open submenu)?
pub fn contains(layout: &Layout, cursor: [i32; 2]) -> bool {
    layout.root.isect_pt_v(cursor) || layout.sub.as_ref().is_some_and(|(p, _)| p.isect_pt_v(cursor))
}

fn draw_panel(buf: &mut [u32], width: usize, height: usize, user: &UserDef, panel: Rcti) {
    let b = user.pixelsize.max(1);
    fill_rect(buf, width, height, panel, OUTLINE.to_xrgb());
    let inner = Rcti { xmin: panel.xmin + b, xmax: panel.xmax - b, ymin: panel.ymin + b, ymax: panel.ymax - b };
    fill_rect(buf, width, height, inner, INNER.to_xrgb());
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, layout: &Layout, cursor: [i32; 2]) {
    let hovered = hit(layout, cursor);
    draw_panel(buf, width, height, user, layout.root);
    for &(rect, c) in &layout.rows {
        let lit = hovered == Some(Hit::Category(c)) || layout.open == Some(c);
        if lit {
            fill_rect(buf, width, height, rect, INNER_SEL.to_xrgb());
        }
        let color = if lit { ITEM_TEXT_SEL } else { ITEM_TEXT };
        font::draw_label(buf, width, height, user, rect, CATEGORIES[c].0, color);
        let arrow = Rcti { xmin: rect.xmax - user.widget_unit + 1, ..rect };
        icons::draw_icon(buf, width, height, user, arrow, Icon::RightArrow, color);
    }
    if let Some((panel, items)) = &layout.sub {
        draw_panel(buf, width, height, user, *panel);
        for &(rect, idx, label) in items {
            let lit = hovered == Some(Hit::Item(idx));
            if lit {
                fill_rect(buf, width, height, rect, INNER_SEL.to_xrgb());
            }
            let icon_rect = Rcti { xmax: rect.xmin + user.widget_unit - 1, ..rect };
            icons::draw_icon(buf, width, height, user, icon_rect, type_icon(label), if lit { ITEM_TEXT_SEL } else { ICON_BLUE });
            let text = Rcti { xmin: rect.xmin + user.widget_unit, ..rect };
            font::draw_label(buf, width, height, user, text, label, if lit { ITEM_TEXT_SEL } else { ITEM_TEXT });
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
    fn every_modifier_is_in_exactly_one_category_in_alphabetical_order() {
        let mut all: Vec<&str> = CATEGORIES.iter().flat_map(|(_, l)| l.iter().copied()).collect();
        assert_eq!(all.len(), Modifier::catalog().len());
        for l in &all {
            assert!(catalog_index(l).is_some(), "{l} is not in the catalog");
        }
        for (_, l) in CATEGORIES {
            assert!(l.windows(2).all(|w| w[0] < w[1]), "alphabetical");
        }
        all.sort();
        all.dedup();
        assert_eq!(all.len(), Modifier::catalog().len(), "no modifier twice");
    }

    #[test]
    fn the_submenu_opens_beside_its_category_inside_the_window() {
        let l = layout(&user(), [300, 500], 1000, 800, Some(1));
        let (panel, items) = l.sub.as_ref().unwrap();
        assert_eq!(items.len(), CATEGORIES[1].1.len());
        assert!(panel.xmin >= l.root.xmax - 10, "to the right of the root");
        assert!(items.iter().all(|(r, _, _)| panel.isect_pt(r.xmin, r.ymin) && panel.isect_pt(r.xmax, r.ymax)));
        assert!(items.windows(2).all(|w| w[0].0.ymin == w[1].0.ymax + 1));
        /* Near the right edge it flips to the left, near the bottom it moves up. */
        let edge = layout(&user(), [900, 40], 1000, 800, Some(0));
        let (p, _) = edge.sub.as_ref().unwrap();
        assert!(p.xmax <= 999 && p.xmin >= 0 && p.ymin >= 0 && edge.root.ymin >= 0);
        assert!(p.xmax < edge.root.xmax, "flipped to the left");
        assert!(layout(&user(), [300, 500], 1000, 800, None).sub.is_none());
    }

    #[test]
    fn hovering_and_clicking_find_the_right_entries() {
        let l = layout(&user(), [300, 500], 1000, 800, Some(0));
        let deform = l.rows[1].0;
        assert_eq!(hit(&l, [deform.cent_x(), deform.cent_y()]), Some(Hit::Category(1)));
        let (_, items) = l.sub.as_ref().unwrap();
        let (r, idx, label) = items[0];
        assert_eq!(label, "Array");
        assert_eq!(hit(&l, [r.cent_x(), r.cent_y()]), Some(Hit::Item(idx)));
        assert_eq!(Modifier::catalog()[idx].label(), "Array");
        assert!(contains(&l, [r.cent_x(), r.cent_y()]) && contains(&l, [deform.cent_x(), deform.cent_y()]));
        assert!(!contains(&l, [0, 0]) && hit(&l, [0, 0]).is_none());
    }
}
