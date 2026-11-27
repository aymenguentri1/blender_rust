//! The dropdown menu of a modifier's panel header (`OBJECT_MT_modifier_*` / the `v` button of `MOD_ui_common.cc`).
//!
//! Blender lists Apply, Duplicate, Copy to Selected, Move to First and Move to Last (plus the shape key entries,
//! which have no counterpart here). It looks like the Add menu (`add_menu`): a dark panel with an outline and one
//! row of `UI_UNIT_Y` per entry, highlighted under the cursor, but without a title. Entries that cannot run
//! (Move to First on the first modifier, Apply on a modifier that is off in the viewport) are drawn dimmed and
//! ignore clicks. Entries, order, icons and the separators follow `modifier_ops_extra_draw` (MOD_ui_common.cc).
//! Deviations: square corners, the separator is blank space, no Apply as Shape Key / Save as Shape Key (no shape
//! keys yet), no Pin to Last, and a click outside only closes it.

use crate::area::{fill_rect, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::scene::Scene;
use crate::theme::Rgba;
use crate::userdef::UserDef;

const OUTLINE: Rgba = Rgba::hex(0x242424ff);
const INNER: Rgba = Rgba::hex(0x181818ff);
const INNER_SEL: Rgba = Rgba::hex(0x4772b3ff);
const ITEM_TEXT: Rgba = Rgba::hex(0xddddddff);
const ITEM_TEXT_SEL: Rgba = Rgba::hex(0xffffffff);
const ITEM_TEXT_OFF: Rgba = Rgba::hex(0x777777ff);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Apply,
    Duplicate,
    CopyToSelected,
    MoveToFirst,
    MoveToLast,
}

impl Item {
    pub const ALL: [Item; 5] = [Item::Apply, Item::Duplicate, Item::CopyToSelected, Item::MoveToFirst, Item::MoveToLast];

    /// The icon of the entry (`ICON_CHECKMARK`, `ICON_DUPLICATE`, none for Copy to Selected, `ICON_TRIA_UP/DOWN`).
    pub fn icon(self) -> Option<Icon> {
        match self {
            Item::Apply => Some(Icon::Checkmark),
            Item::Duplicate => Some(Icon::Duplicate),
            Item::CopyToSelected => None,
            Item::MoveToFirst => Some(Icon::TriaUp),
            Item::MoveToLast => Some(Icon::TriaDown),
        }
    }

    /// The shortcut shown at the right (blender_default.py: `object.modifier_apply` Ctrl A, `modifier_copy` Shift D).
    pub fn shortcut(self) -> Option<&'static str> {
        match self {
            Item::Apply => Some("Ctrl A"),
            Item::Duplicate => Some("Shift D"),
            _ => None,
        }
    }

    /// A separator follows the entry (`layout->separator()` after Copy to Selected).
    pub fn separator_after(self) -> bool {
        self == Item::CopyToSelected
    }

    pub fn label(self) -> &'static str {
        match self {
            Item::Apply => "Apply",
            Item::Duplicate => "Duplicate",
            Item::CopyToSelected => "Copy to Selected",
            Item::MoveToFirst => "Move to First",
            Item::MoveToLast => "Move to Last",
        }
    }

    /// Runs the entry on modifier `i` of the active object. Returns whether the modifier results have to be rebuilt.
    pub fn run(self, scene: &mut Scene, i: usize) -> bool {
        match self {
            Item::Apply => scene.panel_apply_modifier(i),
            Item::Duplicate => scene.panel_duplicate_modifier(i),
            Item::CopyToSelected => scene.panel_copy_modifier_to_selected(i) > 0,
            Item::MoveToFirst => scene.panel_move_modifier_to(i, true),
            Item::MoveToLast => scene.panel_move_modifier_to(i, false),
        }
    }

    /// Can the entry do something for modifier `i` of the active object?
    pub fn enabled(self, scene: &Scene, i: usize) -> bool {
        let Some(o) = scene.active_object() else { return false };
        if i >= o.modifiers.len() {
            return false;
        }
        match self {
            Item::Apply => o.modifier_shown(i),
            Item::Duplicate => true,
            Item::CopyToSelected => scene.objects.iter().any(|t| t.selected && !t.hidden && t.id != o.id && t.mesh().is_some()),
            Item::MoveToFirst => i > 0,
            Item::MoveToLast => i + 1 < o.modifiers.len(),
        }
    }
}

pub struct Layout {
    pub panel: Rcti,
    pub items: Vec<(Rcti, Item, bool)>,
}

/// The panel for a menu of modifier `modifier` opened at window position `pos` (y up) in a `win_w` x `win_h` window.
pub fn layout(user: &UserDef, scene: &Scene, modifier: usize, pos: [i32; 2], win_w: i32, win_h: i32) -> Layout {
    let unit = user.widget_unit;
    let margin = (unit / 5).max(2);
    let widest = Item::ALL.iter().map(|p| font::label_width(user, p.label())).max().unwrap_or(0);
    let width = (widest + 2 * margin + unit).max(8 * unit);
    /* `Layout::separator()` in a menu: `int(7 * UI_SCALE_FAC)`. */
    let sep = (7.0 * user.scale_factor) as i32;
    let seps = Item::ALL.iter().filter(|i| i.separator_after()).count() as i32;
    let height = Item::ALL.len() as i32 * unit + seps * sep + 2 * margin;
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
    let mut top = panel.ymax - margin;
    let mut items = Vec::new();
    for &it in &Item::ALL {
        let r = Rcti { xmin: panel.xmin + margin, xmax: panel.xmax - margin, ymax: top, ymin: top - unit + 1 };
        items.push((r, it, it.enabled(scene, modifier)));
        top -= unit + if it.separator_after() { sep } else { 0 };
    }
    Layout { panel, items }
}

/// The enabled entry under the window position `cursor`.
pub fn item_at(layout: &Layout, cursor: [i32; 2]) -> Option<Item> {
    layout.items.iter().find(|(r, _, on)| *on && r.isect_pt_v(cursor)).map(|&(_, it, _)| it)
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, layout: &Layout, cursor: [i32; 2]) {
    let unit = user.widget_unit;
    let margin = (unit / 5).max(2);
    let b = user.pixelsize.max(1);
    fill_rect(buf, width, height, layout.panel, OUTLINE.to_xrgb());
    let inner = Rcti {
        xmin: layout.panel.xmin + b,
        xmax: layout.panel.xmax - b,
        ymin: layout.panel.ymin + b,
        ymax: layout.panel.ymax - b,
    };
    fill_rect(buf, width, height, inner, INNER.to_xrgb());
    let hover = item_at(layout, cursor);
    for &(rect, item, on) in &layout.items {
        let selected = hover == Some(item);
        if selected {
            fill_rect(buf, width, height, rect, INNER_SEL.to_xrgb());
        }
        let color = if !on { ITEM_TEXT_OFF } else if selected { ITEM_TEXT_SEL } else { ITEM_TEXT };
        let text = if let Some(icon) = item.icon() {
            icons::draw_icon(buf, width, height, user, Rcti { xmax: rect.xmin + unit - 1, ..rect }, icon, color);
            Rcti { xmin: rect.xmin + unit, ..rect }
        } else {
            rect
        };
        font::draw_label(buf, width, height, user, text, item.label(), color);
        if let Some(key) = item.shortcut() {
            let pad = (rect.size_x() + 1 - font::string_width(user, key) - margin).max(0);
            font::draw_label_pad(buf, width, height, user, rect, key, if selected { ITEM_TEXT_SEL } else { ITEM_TEXT_OFF }, pad);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modifiers::Modifier;

    fn scene_with(n: usize) -> Scene {
        let mut s = Scene::startup();
        for k in 0..n {
            s.panel_add_modifier(if k == 0 { Modifier::array(2) } else { Modifier::mirror(0) });
        }
        s
    }

    #[test]
    fn the_menu_lists_five_rows_inside_the_window() {
        let s = scene_with(2);
        let l = layout(&UserDef::from_window_scale(1.0), &s, 0, [300, 500], 1000, 800);
        assert_eq!(l.items.len(), 5);
        assert!(l.items.iter().all(|(r, _, _)| l.panel.isect_pt(r.xmin, r.ymin) && l.panel.isect_pt(r.xmax, r.ymax)));
        assert!(l.items.windows(2).all(|w| w[0].0.ymin > w[1].0.ymax));
        /* Near the window edge the panel moves back in. */
        let edge = layout(&UserDef::from_window_scale(1.0), &s, 0, [995, 5], 1000, 800);
        assert!(edge.panel.xmax <= 999 && edge.panel.ymin >= 0);
    }

    #[test]
    fn icons_shortcuts_and_the_separator_follow_modifier_ops_extra_draw() {
        let icons: Vec<Option<Icon>> = Item::ALL.iter().map(|i| i.icon()).collect();
        assert_eq!(icons, [Some(Icon::Checkmark), Some(Icon::Duplicate), None, Some(Icon::TriaUp), Some(Icon::TriaDown)]);
        assert_eq!((Item::Apply.shortcut(), Item::Duplicate.shortcut(), Item::MoveToLast.shortcut()), (Some("Ctrl A"), Some("Shift D"), None));
        let user = UserDef::from_window_scale(1.0);
        let l = layout(&user, &scene_with(2), 0, [300, 500], 1000, 800);
        let gap = |a: usize| l.items[a].0.ymin - l.items[a + 1].0.ymax - 1;
        assert_eq!((gap(0), gap(1), gap(2), gap(3)), (0, 0, 7, 0), "one 7 px separator after Copy to Selected");
    }

    #[test]
    fn entries_that_cannot_run_are_disabled() {
        let s = scene_with(2);
        assert!(!Item::MoveToFirst.enabled(&s, 0));
        assert!(Item::MoveToLast.enabled(&s, 0));
        assert!(Item::MoveToFirst.enabled(&s, 1));
        assert!(!Item::MoveToLast.enabled(&s, 1));
        assert!(!Item::CopyToSelected.enabled(&s, 0), "no other selected mesh");
        assert!(!Item::Duplicate.enabled(&s, 5), "no such modifier");
        let l = layout(&UserDef::from_window_scale(1.0), &s, 0, [300, 500], 1000, 800);
        let first = l.items.iter().find(|(_, it, _)| *it == Item::MoveToFirst).unwrap().0;
        assert_eq!(item_at(&l, [first.cent_x(), first.cent_y()]), None, "a disabled row ignores the cursor");
        let dup = l.items.iter().find(|(_, it, _)| *it == Item::Duplicate).unwrap().0;
        assert_eq!(item_at(&l, [dup.cent_x(), dup.cent_y()]), Some(Item::Duplicate));
    }

    #[test]
    fn entries_edit_the_stack() {
        let mut s = scene_with(3);
        let labels = |s: &Scene| s.active_object().unwrap().modifiers.iter().map(|m| m.label()).collect::<Vec<_>>();
        assert!(Item::MoveToLast.run(&mut s, 0));
        assert_eq!(labels(&s), ["Mirror", "Mirror", "Array"]);
        assert!(Item::MoveToFirst.run(&mut s, 2));
        assert_eq!(labels(&s), ["Array", "Mirror", "Mirror"]);
        assert_eq!(s.active_object().unwrap().active_modifier, 0, "it stays the active modifier");
        assert!(!Item::MoveToFirst.run(&mut s, 0), "already first");
        assert!(Item::Duplicate.run(&mut s, 0));
        assert_eq!(s.active_object().unwrap().modifiers.len(), 4);
        assert!(Item::Apply.run(&mut s, 0));
        assert_eq!(s.active_object().unwrap().modifiers.len(), 3);
    }

    #[test]
    fn copy_to_selected_appends_the_modifier_to_the_other_selected_meshes() {
        let mut s = scene_with(2);
        s.panel_rename_modifier(0, "Row");
        s.panel_toggle_viewport(0);
        let active = s.active.unwrap();
        let other = s.add_primitive(crate::mesh::Primitive::Cube, [3.0, 0.0, 0.0]);
        s.objects.iter_mut().for_each(|o| o.selected = true);
        s.active = Some(active);
        assert!(Item::CopyToSelected.enabled(&s, 0));
        assert_eq!(s.panel_copy_modifier_to_selected(0), 1);
        let o = &s.objects[s.index_of(other).unwrap()];
        assert_eq!(o.modifiers, [Modifier::array(2)]);
        assert_eq!(o.modifier_name(0).as_deref(), Some("Row"), "the name comes along");
        assert!(!o.modifier_shown(0), "and the viewport flag");
        assert_eq!(s.active_object().unwrap().modifiers.len(), 2, "the source is unchanged");
        assert!(s.undo());
        assert!(s.objects[s.index_of(other).unwrap()].modifiers.is_empty());
    }
}
