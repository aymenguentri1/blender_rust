//! The `Add Modifier` popup of the Modifiers panel (`OBJECT_MT_modifier_add`, `properties_data_modifier.py`).
//!
//! Blender's button opens a menu with one submenu per category (Edit, Generate, Deform, Normals, Physics, ...), each
//! listing its modifiers in the order of the python menus with their icons. Every entry of the mesh menus is listed;
//! those without a modifier in this port are inert, and icons without a drawn glyph use the wrench. The look follows the other popups
//! (`modifier_menu`, `add_menu`): a dark panel with an outline, one row of `UI_UNIT_Y` per entry, the entry under the
//! cursor highlighted, an arrow at the right of a category. The submenu opens beside the hovered category and stays
//! open while the cursor is inside the menu. Deviations: square corners, no search field (Blender 4.x types to
//! search), no keyboard navigation, and a click outside only closes it.
//!
//! Layout and hit-testing are plain functions of the position, so they are tested without a window.

use crate::area::{fill_rect, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::modifiers::Modifier;
use crate::theme::Rgba;
use crate::userdef::UserDef;

const OUTLINE: Rgba = Rgba::hex(0x242424ff);
const INNER: Rgba = Rgba::hex(0x181818ff);
const INNER_SEL: Rgba = Rgba::hex(0x4772b3ff);
const ITEM_TEXT: Rgba = Rgba::hex(0xddddddff);
const ITEM_TEXT_SEL: Rgba = Rgba::hex(0xffffffff);
const ICON_BLUE: Rgba = Rgba::hex(0x8ab4ebff);

/// One entry of a category: the name Blender shows (`rna_modifier.cc`), the `Modifier::label` of the modifier of this
/// port that it adds (`None`: not implemented yet, listed but inert) and its `ICON_MOD_*` name.
pub type Entry = (&'static str, Option<&'static str>, &'static str);

/// `OBJECT_MT_modifier_add_edit` .. `_physics` for a mesh object, entries in the order of `properties_data_modifier.py`.
/// "Array" is the geometry-nodes asset (`operator_modifier_add_asset`), "Array (Legacy)" the `ARRAY` modifier.
pub const CATEGORIES: [(&str, &[Entry]); 5] = [
    ("Edit", &[
        ("Data Transfer", None, "MOD_DATA_TRANSFER"),
        ("Mesh Cache", None, "MOD_MESHDEFORM"),
        ("Mesh Sequence Cache", None, "MOD_MESHDEFORM"),
        ("UV Project", None, "MOD_UVPROJECT"),
        ("UV Warp", None, "MOD_UVPROJECT"),
        ("Vertex Weight Edit", None, "MOD_VERTEX_WEIGHT"),
        ("Vertex Weight Mix", None, "MOD_VERTEX_WEIGHT"),
        ("Vertex Weight Proximity", None, "MOD_VERTEX_WEIGHT"),
    ]),
    ("Generate", &[
        ("Array", None, "MOD_ARRAY"),
        ("Array (Legacy)", Some("Array"), "MOD_ARRAY"),
        ("Bevel", Some("Bevel"), "MOD_BEVEL"),
        ("Boolean", Some("Boolean"), "MOD_BOOLEAN"),
        ("Build", Some("Build"), "MOD_BUILD"),
        ("Curve to Tube", None, "MOD_CURVE_TO_TUBE"),
        ("Decimate", Some("Decimate"), "MOD_DECIM"),
        ("Edge Split", Some("Edge Split"), "MOD_EDGESPLIT"),
        ("Mask", None, "MOD_MASK"),
        ("Mirror", Some("Mirror"), "MOD_MIRROR"),
        ("Multiresolution", None, "MOD_MULTIRES"),
        ("Remesh", Some("Remesh"), "MOD_REMESH"),
        ("Scatter on Surface", None, "MOD_SCATTER_ON_SURFACE"),
        ("Screw", Some("Screw"), "MOD_SCREW"),
        ("Skin", None, "MOD_SKIN"),
        ("Solidify", Some("Solidify"), "MOD_SOLIDIFY"),
        ("Subdivision Surface", Some("Subdivision Surface"), "MOD_SUBSURF"),
        ("Triangulate", Some("Triangulate"), "MOD_TRIANGULATE"),
        ("Volume to Mesh", None, "VOLUME_DATA"),
        ("Weld", Some("Weld"), "AUTOMERGE_OFF"),
        ("Wireframe", Some("Wireframe"), "MOD_WIREFRAME"),
    ]),
    ("Deform", &[
        ("Armature", None, "MOD_ARMATURE"),
        ("Cast", Some("Cast"), "MOD_CAST"),
        ("Curve", None, "MOD_CURVE"),
        ("Displace", Some("Displace"), "MOD_DISPLACE"),
        ("Hook", None, "HOOK"),
        ("Laplacian Deform", None, "MOD_MESHDEFORM"),
        ("Lattice", None, "MOD_LATTICE"),
        ("Mesh Deform", None, "MOD_MESHDEFORM"),
        ("Shrinkwrap", Some("Shrinkwrap"), "MOD_SHRINKWRAP"),
        ("Simple Deform", Some("Simple Deform"), "MOD_SIMPLEDEFORM"),
        ("Smooth", Some("Smooth"), "MOD_SMOOTH"),
        ("Smooth Corrective", Some("Corrective Smooth"), "MOD_SMOOTH"),
        ("Smooth Laplacian", Some("Laplacian Smooth"), "MOD_SMOOTH"),
        ("Surface Deform", None, "MOD_MESHDEFORM"),
        ("Warp", None, "MOD_WARP"),
        ("Wave", Some("Wave"), "MOD_WAVE"),
    ]),
    ("Normals", &[
        ("Normal Edit", None, "MOD_NORMALEDIT"),
        ("Weighted Normal", None, "MOD_NORMALEDIT"),
    ]),
    ("Physics", &[
        ("Cloth", None, "MOD_CLOTH"),
        ("Collision", None, "MOD_PHYSICS"),
        ("Dynamic Paint", None, "MOD_DYNAMICPAINT"),
        ("Explode", None, "MOD_EXPLODE"),
        ("Fluid", None, "MOD_FLUIDSIM"),
        ("Ocean", None, "MOD_OCEAN"),
        ("Particle Instance", None, "MOD_PARTICLE_INSTANCE"),
        ("Particle System", None, "MOD_PARTICLES"),
        ("Soft Body", None, "MOD_SOFT"),
    ]),
];

/// The glyph of an `ICON_*` name of the table; names without a drawn glyph get the generic wrench.
fn icon_of(name: &str) -> Icon {
    match name {
        "MOD_ARRAY" => Icon::ModArray,
        "MOD_MIRROR" => Icon::ModMirror,
        "MOD_SOLIDIFY" => Icon::ModSolidify,
        "MOD_BEVEL" => Icon::ModBevel,
        "MOD_SMOOTH" => Icon::ModSmooth,
        "MOD_SIMPLEDEFORM" | "MOD_CAST" => Icon::ModDeform,
        "MOD_TRIANGULATE" => Icon::ModTriangulate,
        "MOD_BOOLEAN" => Icon::ModBoolean,
        "MOD_WIREFRAME" => Icon::ModWireframe,
        "MOD_DECIM" => Icon::ModDecimate,
        "MOD_REMESH" => Icon::ModRemesh,
        _ => Icon::ModWrench,
    }
}

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
    /// An entry Blender lists but this port has no modifier for: highlighted, does nothing.
    Inert,
}

/// The open submenu: its panel and its entries (`catalog index`, label).
type SubMenu = (Rcti, Vec<(Rcti, Option<usize>, &'static Entry)>);

pub struct Layout {
    pub root: Rcti,
    pub rows: Vec<(Rcti, usize)>,
    /// The open submenu: its panel and its entries (`catalog index`, label).
    pub sub: Option<SubMenu>,
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
        let widest = labels.iter().map(|e| font::label_width(user, e.0)).max().unwrap_or(0);
        let sw = (widest + 2 * m.margin + 2 * m.unit).max(8 * m.unit);
        /* Beside the category row, flipped to the left when it would leave the window. */
        let right = root.xmax - m.margin;
        let sx = if right + sw - 1 > win_w - 1 { (root.xmin - sw + m.margin).max(0) } else { right };
        let panel = panel_for(&m, labels.len(), sw, sx, rows[c].0.ymax + m.margin, win_h);
        let items = labels.iter().enumerate().map(|(i, e)| (row(&panel, i), e.1.and_then(catalog_index), e)).collect();
        (panel, items)
    });
    Layout { root, rows, sub, open }
}

/// The row or entry under the window position `cursor`.
pub fn hit(layout: &Layout, cursor: [i32; 2]) -> Option<Hit> {
    if let Some((_, items)) = &layout.sub {
        if let Some((_, idx, _)) = items.iter().find(|(r, _, _)| r.isect_pt_v(cursor)) {
            return Some(idx.map_or(Hit::Inert, Hit::Item));
        }
    }
    layout.rows.iter().find(|(r, _)| r.isect_pt_v(cursor)).map(|&(_, c)| Hit::Category(c))
}

fn hit_rect(layout: &Layout, cursor: [i32; 2]) -> Option<Rcti> {
    layout.sub.as_ref()?.1.iter().find(|(r, _, _)| r.isect_pt_v(cursor)).map(|(r, _, _)| *r)
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
        for &(rect, idx, entry) in items {
            let lit = hovered == Some(idx.map_or(Hit::Inert, Hit::Item)) && hit_rect(layout, cursor) == Some(rect);
            if lit {
                fill_rect(buf, width, height, rect, INNER_SEL.to_xrgb());
            }
            let icon_rect = Rcti { xmax: rect.xmin + user.widget_unit - 1, ..rect };
            icons::draw_icon(buf, width, height, user, icon_rect, icon_of(entry.2), if lit { ITEM_TEXT_SEL } else { ICON_BLUE });
            let text = Rcti { xmin: rect.xmin + user.widget_unit, ..rect };
            font::draw_label(buf, width, height, user, text, entry.0, if lit { ITEM_TEXT_SEL } else { ITEM_TEXT });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserDef {
        UserDef::from_window_scale(1.0)
    }

    fn names(c: usize) -> Vec<&'static str> {
        CATEGORIES[c].1.iter().map(|e| e.0).collect()
    }

    #[test]
    fn the_categories_and_items_follow_the_python_menus() {
        let cats: Vec<&str> = CATEGORIES.iter().map(|c| c.0).collect();
        assert_eq!(cats, ["Edit", "Generate", "Deform", "Normals", "Physics"]);
        assert_eq!(names(3), ["Normal Edit", "Weighted Normal"]);
        assert_eq!(names(4).first(), Some(&"Cloth"));
        assert_eq!(names(4).last(), Some(&"Soft Body"));
        assert_eq!(&names(1)[..3], ["Array", "Array (Legacy)", "Bevel"]);
        assert_eq!(names(1).len(), 21);
        assert_eq!(names(2).len(), 16);
        assert_eq!(names(0).len(), 8);
    }

    #[test]
    fn every_implemented_modifier_is_reachable_exactly_once() {
        let mut hits: Vec<&str> = CATEGORIES.iter().flat_map(|(_, l)| l.iter().filter_map(|e| e.1)).collect();
        for l in &hits {
            assert!(catalog_index(l).is_some(), "{l} is not in the catalog");
        }
        hits.sort();
        hits.dedup();
        assert_eq!(hits.len(), Modifier::catalog().len(), "all catalog modifiers, none twice");
    }

    #[test]
    fn the_submenu_opens_beside_its_category_inside_the_window() {
        let l = layout(&user(), [300, 500], 1000, 800, Some(2));
        let (panel, items) = l.sub.as_ref().unwrap();
        assert_eq!(items.len(), CATEGORIES[2].1.len());
        assert!(panel.xmin >= l.root.xmax - 10, "to the right of the root");
        assert!(items.iter().all(|(r, _, _)| panel.isect_pt(r.xmin, r.ymin) && panel.isect_pt(r.xmax, r.ymax)));
        assert!(items.windows(2).all(|w| w[0].0.ymin == w[1].0.ymax + 1));
        /* Near the right edge it flips to the left, near the bottom it moves up. */
        let edge = layout(&user(), [900, 40], 1000, 800, Some(1));
        let (p, _) = edge.sub.as_ref().unwrap();
        assert!(p.xmax <= 999 && p.xmin >= 0 && p.ymin >= 0 && edge.root.ymin >= 0);
        assert!(p.xmax < edge.root.xmax, "flipped to the left");
        assert!(layout(&user(), [300, 500], 1000, 800, None).sub.is_none());
    }

    #[test]
    fn hovering_and_clicking_find_the_right_entries() {
        let l = layout(&user(), [300, 500], 1000, 800, Some(1));
        let deform = l.rows[2].0;
        assert_eq!(hit(&l, [deform.cent_x(), deform.cent_y()]), Some(Hit::Category(2)));
        let (_, items) = l.sub.as_ref().unwrap();
        assert_eq!(hit(&l, [items[0].0.cent_x(), items[0].0.cent_y()]), Some(Hit::Inert), "Array is the asset");
        let (r, idx, entry) = items[1];
        let idx = idx.unwrap();
        assert_eq!(entry.0, "Array (Legacy)");
        assert_eq!(hit(&l, [r.cent_x(), r.cent_y()]), Some(Hit::Item(idx)));
        assert_eq!(Modifier::catalog()[idx].label(), "Array");
        assert!(contains(&l, [r.cent_x(), r.cent_y()]) && contains(&l, [deform.cent_x(), deform.cent_y()]));
        assert!(!contains(&l, [0, 0]) && hit(&l, [0, 0]).is_none());
    }
}
