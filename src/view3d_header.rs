//! The 3D viewport header (`VIEW3D_HT_header`, scripts/startup/bl_ui/space_view3d.py).
//!
//! Left to right: the editor type dropdown (`template_header()`), the mode dropdown
//! (`object.mode_set`), the pull-down menus (`VIEW3D_MT_editor_menus`: View, Select, Add, Object),
//! then the transform cluster (orientation, pivot point, snapping, proportional editing); right
//! aligned the overlays and X-ray toggles and the four shading buttons (`draw_right()`-style
//! placement of `VIEW3D_HT_header.draw()`).
//!
//! Only the toggles that exist in this port do something (see `Action`); the rest is drawn like
//! Blender draws it. Deviation: when the cluster of transform buttons does not fit, it is dropped
//! as a whole, where Blender collapses the header region with a scroll.

use crate::area::{global_header_layout_start, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::theme::{self, Rgba};
use crate::transform::PivotPoint;
use crate::userdef::UserDef;
use crate::widget_draw::{draw_round_box, draw_round_box_sides};

/// What a header button does when it is pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    EditorType,
    Mode,
    MenuView,
    MenuSelect,
    MenuAdd,
    MenuObject,
    Orientation,
    Pivot,
    Snap,
    SnapOptions,
    Proportional,
    ProportionalOptions,
    Visibility,
    Gizmo,
    GizmoOptions,
    Overlays,
    OverlaysOptions,
    Xray,
    ShadeWire,
    ShadeSolid,
    ShadeMaterial,
    ShadeRendered,
    ShadeOptions,
}

/// The state the header shows.
#[derive(Clone, Copy, Debug)]
pub struct HeaderState {
    pub wire: bool,
    pub xray: bool,
    pub overlays: bool,
    pub pivot: PivotPoint,
    /// The mode dropdown reads `Object Mode`, `Edit Mode` or `Sculpt Mode` (`rna_enum_object_mode_items`, rna_object.cc:39).
    pub mode: crate::object_mode::ObjectMode,
}

/// How an item is drawn.
#[derive(Clone, Debug)]
enum Look {
    /// A pull-down menu label, no box (`wcol_pulldown`).
    Menu(&'static str),
    /// A dropdown box with an icon, an optional label and the arrow.
    Dropdown { icon: Icon, label: Option<String> },
    /// An icon button; `on` is the pressed state. `left` / `right` tell which sides touch a neighbour.
    Toggle { icon: Icon, on: bool, dim: bool, left: bool, right: bool },
    /// The small arrow button next to a toggle.
    Arrow { left: bool, right: bool },
}

#[derive(Clone, Debug)]
pub struct Item {
    pub action: Action,
    pub rect: Rcti,
    look: Look,
}

/// Where the next item of a row goes.
struct Cursor {
    x: i32,
    y0: i32,
    y1: i32,
    space: i32,
}

impl Cursor {
    fn place(&mut self, width: i32, gap_after: bool) -> Rcti {
        let r = Rcti { xmin: self.x, xmax: self.x + width - 1, ymin: self.y0, ymax: self.y1 };
        self.x += width + if gap_after { self.space } else { 0 };
        r
    }
}

fn unit(user: &UserDef) -> i32 {
    user.widget_unit
}

/// Width of a dropdown: icon cell, label, arrow cell.
fn dropdown_width(user: &UserDef, label: Option<&str>) -> i32 {
    let icon_cell = unit(user);
    let arrow_cell = unit(user) * 3 / 4;
    let text = label.map(|l| font::string_width(user, l) + (user.scale_factor * 4.0) as i32).unwrap_or(0);
    icon_cell + text + arrow_cell
}

/// The pivot point shown by the icon button.
pub fn pivot_icon(p: PivotPoint) -> Icon {
    match p {
        PivotPoint::Median => Icon::PivotMedian,
        PivotPoint::Cursor => Icon::PivotCursor,
        PivotPoint::Individual => Icon::PivotIndividual,
    }
}

/// Lays out every header button for the header region `header` (window coordinates).
pub fn layout(user: &UserDef, header: Rcti, state: &HeaderState) -> Vec<Item> {
    let (offset, top_y, height) = global_header_layout_start(user, header.size_y() + 1);
    let y0 = header.ymin + top_y - height;
    let y1 = header.ymin + top_y - 1;
    let space = user.button_space_x;
    let mut items = Vec::new();

    /* Left: editor type, mode, menus. */
    let mut c = Cursor { x: header.xmin + offset, y0, y1, space };
    let w = dropdown_width(user, None);
    items.push(Item { action: Action::EditorType, rect: c.place(w, true), look: Look::Dropdown { icon: Icon::View3D, label: None } });
    let (label, mode_icon) = match state.mode {
        crate::object_mode::ObjectMode::Sculpt => ("Sculpt Mode".to_string(), Icon::SculptMode),
        crate::object_mode::ObjectMode::Edit => ("Edit Mode".to_string(), Icon::EditMode),
        crate::object_mode::ObjectMode::Object => ("Object Mode".to_string(), Icon::ObjectMode),
    };
    let w = dropdown_width(user, Some(&label));
    items.push(Item { action: Action::Mode, rect: c.place(w, true), look: Look::Dropdown { icon: mode_icon, label: Some(label) } });
    for (action, name) in [
        (Action::MenuView, "View"),
        (Action::MenuSelect, "Select"),
        (Action::MenuAdd, "Add"),
        (Action::MenuObject, "Object"),
    ] {
        let w = font::label_width(user, name);
        items.push(Item { action, rect: c.place(w, false), look: Look::Menu(name) });
    }
    let left_end = c.x + space;

    /* Right: built from the right edge so that the cluster stays right aligned. */
    let u = unit(user);
    let toggle_w = u * 3 / 2;
    let arrow_w = u * 3 / 4;
    let pair_w = toggle_w + arrow_w;
    let shade_w = 4 * toggle_w + arrow_w;
    let vis_w = dropdown_width(user, None);
    let right_total = vis_w + space + pair_w + space + pair_w + space + toggle_w + space + shade_w;
    let right_x = header.xmax + 1 - offset - right_total;

    /* Middle: the transform cluster, only when it fits between the two. */
    let orient_label = "Global".to_string();
    let orient_w = dropdown_width(user, Some(&orient_label));
    let pivot_w = u + arrow_w;
    let cluster_total = orient_w + space + pivot_w + space + pair_w + space + pair_w;
    let cluster_x = left_end + space * 2;
    if cluster_x + cluster_total + space * 2 <= right_x {
        let mut c = Cursor { x: cluster_x, y0, y1, space };
        items.push(Item { action: Action::Orientation, rect: c.place(orient_w, true), look: Look::Dropdown { icon: Icon::OrientationGlobal, label: Some(orient_label) } });
        items.push(Item { action: Action::Pivot, rect: c.place(pivot_w, true), look: Look::Dropdown { icon: pivot_icon(state.pivot), label: None } });
        items.push(Item { action: Action::Snap, rect: c.place(toggle_w, false), look: Look::Toggle { icon: Icon::Snap, on: false, dim: false, left: false, right: true } });
        items.push(Item { action: Action::SnapOptions, rect: c.place(arrow_w, true), look: Look::Arrow { left: true, right: false } });
        items.push(Item { action: Action::Proportional, rect: c.place(toggle_w, false), look: Look::Toggle { icon: Icon::Proportional, on: false, dim: false, left: false, right: true } });
        items.push(Item { action: Action::ProportionalOptions, rect: c.place(arrow_w, false), look: Look::Arrow { left: true, right: false } });
    }

    let mut c = Cursor { x: right_x, y0, y1, space };
    /* `popover(VIEW3D_PT_object_type_visibility, icon_value=icon_from_show_object_viewport)` and the `show_gizmo`
     * toggle with its `VIEW3D_PT_gizmo_display` popover (space_view3d.py:1103). */
    items.push(Item { action: Action::Visibility, rect: c.place(vis_w, true), look: Look::Dropdown { icon: Icon::Eye, label: None } });
    items.push(Item { action: Action::Gizmo, rect: c.place(toggle_w, false), look: Look::Toggle { icon: Icon::Gizmo, on: true, dim: false, left: false, right: true } });
    items.push(Item { action: Action::GizmoOptions, rect: c.place(arrow_w, true), look: Look::Arrow { left: true, right: false } });
    items.push(Item { action: Action::Overlays, rect: c.place(toggle_w, false), look: Look::Toggle { icon: Icon::Overlays, on: state.overlays, dim: false, left: false, right: true } });
    items.push(Item { action: Action::OverlaysOptions, rect: c.place(arrow_w, true), look: Look::Arrow { left: true, right: false } });
    /* In wire shading `V3D_SHADING_XRAY_WIREFRAME` is not modelled: dimmed and inert. */
    items.push(Item { action: Action::Xray, rect: c.place(toggle_w, true), look: Look::Toggle { icon: Icon::Xray, on: state.xray && !state.wire, dim: state.wire, left: false, right: false } });
    let solid = !state.wire;
    for (i, (action, icon, on)) in [
        (Action::ShadeWire, Icon::ShadingWire, state.wire),
        (Action::ShadeSolid, Icon::ShadingSolid, solid),
        (Action::ShadeMaterial, Icon::ShadingMaterial, false),
        (Action::ShadeRendered, Icon::ShadingRendered, false),
    ]
    .into_iter()
    .enumerate()
    {
        items.push(Item { action, rect: c.place(toggle_w, false), look: Look::Toggle { icon, on, dim: false, left: i > 0, right: true } });
    }
    items.push(Item { action: Action::ShadeOptions, rect: c.place(arrow_w, false), look: Look::Arrow { left: true, right: false } });
    items
}

/// The action under `xy` (window coordinates), if any.
pub fn action_at(items: &[Item], xy: [i32; 2]) -> Option<Action> {
    items.iter().find(|i| i.rect.isect_pt_v(xy)).map(|i| i.action)
}

/// The window rect of the item of `action`, for placing a menu under it.
pub fn rect_of(items: &[Item], action: Action) -> Option<Rcti> {
    items.iter().find(|i| i.action == action).map(|i| i.rect)
}

fn lighten(c: Rgba, by: u8) -> Rgba {
    Rgba { r: c.r.saturating_add(by), g: c.g.saturating_add(by), b: c.b.saturating_add(by), a: c.a }
}

fn dim(c: Rgba) -> Rgba {
    Rgba { a: c.a / 2, ..c }
}

/// Icon colour of a header button: `TH_TEXT` of the tool widget colours.
const ICON_COLOR: Rgba = Rgba::hex(0xe6e6e6ff);
const ICON_COLOR_SEL: Rgba = Rgba::hex(0xffffffff);

/// Draws every item; `hover` is the action under the cursor.
pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, items: &[Item], hover: Option<Action>) {
    for item in items {
        let hot = hover == Some(item.action);
        match &item.look {
            Look::Menu(name) => {
                if hot {
                    draw_round_box(buf, width, height, user, item.rect, Rgba::hex(0xffffff1a), Rgba::hex(0x00000000), theme::WCOL_TOOL_ROUNDNESS);
                }
                font::draw_label(buf, width, height, user, item.rect, name, theme::WCOL_PULLDOWN_TEXT);
            }
            Look::Dropdown { icon, label } => {
                let inner = if hot { lighten(theme::WCOL_TOOL_INNER, 16) } else { theme::WCOL_TOOL_INNER };
                draw_round_box(buf, width, height, user, item.rect, inner, theme::WCOL_TOOL_OUTLINE, theme::WCOL_TOOL_ROUNDNESS);
                let u = unit(user);
                let icon_cell = Rcti { xmax: item.rect.xmin + u - 1, ..item.rect };
                icons::draw_icon(buf, width, height, user, icon_cell, *icon, ICON_COLOR);
                if let Some(label) = label {
                    let text_rect = Rcti { xmin: item.rect.xmin + u, ..item.rect };
                    font::draw_label_pad(buf, width, height, user, text_rect, label, theme::WCOL_MENU_TEXT, 0);
                }
                let arrow_cell = Rcti { xmin: item.rect.xmax + 1 - u * 3 / 4, ..item.rect };
                icons::draw_icon(buf, width, height, user, arrow_cell, Icon::DownArrow, ICON_COLOR);
            }
            Look::Toggle { icon, on, dim: dimmed, left, right } => {
                let mut inner = if *on { theme::WCOL_TOGGLE_INNER_SEL } else { theme::WCOL_TOOL_INNER };
                if hot && !dimmed {
                    inner = lighten(inner, 16);
                }
                let mut outline = theme::WCOL_TOOL_OUTLINE;
                if *dimmed {
                    inner = dim(inner);
                    outline = dim(outline);
                }
                draw_round_box_sides(buf, width, height, user, item.rect, inner, outline, theme::WCOL_TOOL_ROUNDNESS, !*left, !*right);
                let mut col = if *on { ICON_COLOR_SEL } else { ICON_COLOR };
                if *dimmed {
                    col = dim(col);
                }
                icons::draw_icon(buf, width, height, user, item.rect, *icon, col);
            }
            Look::Arrow { left, right } => {
                let inner = if hot { lighten(theme::WCOL_TOOL_INNER, 16) } else { theme::WCOL_TOOL_INNER };
                draw_round_box_sides(buf, width, height, user, item.rect, inner, theme::WCOL_TOOL_OUTLINE, theme::WCOL_TOOL_ROUNDNESS, !*left, !*right);
                icons::draw_icon(buf, width, height, user, item.rect, Icon::DownArrow, ICON_COLOR);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> HeaderState {
        HeaderState { wire: false, xray: false, overlays: true, pivot: PivotPoint::Median, mode: crate::object_mode::ObjectMode::Object }
    }

    fn header() -> Rcti {
        Rcti { xmin: 2, xmax: 1195, ymin: 700, ymax: 725 }
    }

    /* Nothing overlaps and everything stays inside the header. */
    #[test]
    fn items_do_not_overlap_and_fit() {
        let user = UserDef::from_window_scale(1.0);
        let h = header();
        let items = layout(&user, h, &state());
        for (i, a) in items.iter().enumerate() {
            assert!(a.rect.xmin >= h.xmin && a.rect.xmax <= h.xmax, "{:?}", a.action);
            assert!(a.rect.ymin >= h.ymin && a.rect.ymax <= h.ymax, "{:?}", a.action);
            for b in &items[i + 1..] {
                assert!(a.rect.isect(&b.rect).is_none(), "{:?} overlaps {:?}", a.action, b.action);
            }
        }
    }

    /* The editor type button comes first, the mode dropdown second, the menus after them. */
    #[test]
    fn left_group_order() {
        let user = UserDef::from_window_scale(1.0);
        let items = layout(&user, header(), &state());
        let x = |a| rect_of(&items, a).unwrap().xmin;
        assert!(x(Action::EditorType) < x(Action::Mode));
        assert!(x(Action::Mode) < x(Action::MenuView));
        assert!(x(Action::MenuView) < x(Action::MenuSelect));
        assert!(x(Action::MenuAdd) < x(Action::MenuObject));
        assert!(x(Action::MenuObject) < x(Action::Orientation));
        assert!(x(Action::Orientation) < x(Action::Overlays));
    }

    /* The shading buttons end at the right edge, minus the header offset. */
    #[test]
    fn right_group_is_right_aligned() {
        let user = UserDef::from_window_scale(1.0);
        let h = header();
        let items = layout(&user, h, &state());
        let last = rect_of(&items, Action::ShadeOptions).unwrap();
        assert_eq!(last.xmax, h.xmax - 4);
    }

    /* In a narrow header the transform cluster is dropped, the rest stays. */
    #[test]
    fn narrow_header_drops_the_cluster() {
        let user = UserDef::from_window_scale(1.0);
        let h = Rcti { xmin: 2, xmax: 760, ymin: 700, ymax: 725 };
        let items = layout(&user, h, &state());
        assert!(rect_of(&items, Action::Orientation).is_none());
        assert!(rect_of(&items, Action::ShadeSolid).is_some());
    }

    #[test]
    fn hit_test_finds_the_button() {
        let user = UserDef::from_window_scale(1.0);
        let items = layout(&user, header(), &state());
        let r = rect_of(&items, Action::ShadeWire).unwrap();
        assert_eq!(action_at(&items, [r.cent_x(), r.cent_y()]), Some(Action::ShadeWire));
        assert_eq!(action_at(&items, [0, 0]), None);
    }

    /* Wire shading highlights Wire, solid shading highlights Solid. */
    #[test]
    fn shading_buttons_follow_the_state() {
        let user = UserDef::from_window_scale(1.0);
        let on = |s: &HeaderState, a: Action| {
            layout(&user, header(), s).into_iter().find(|i| i.action == a).map(|i| match i.look {
                Look::Toggle { on, .. } => on,
                _ => false,
            })
        };
        let mut s = state();
        assert_eq!(on(&s, Action::ShadeSolid), Some(true));
        assert_eq!(on(&s, Action::ShadeWire), Some(false));
        s.wire = true;
        assert_eq!(on(&s, Action::ShadeSolid), Some(false));
        assert_eq!(on(&s, Action::ShadeWire), Some(true));
    }

    /* Right cluster: visibility, gizmo, overlays, x-ray, shading (VIEW3D_HT_header.draw()). */
    #[test]
    fn right_cluster_order() {
        let user = UserDef::from_window_scale(1.0);
        let items = layout(&user, header(), &state());
        let x = |a| rect_of(&items, a).unwrap().xmin;
        assert!(x(Action::Visibility) < x(Action::Gizmo));
        assert!(x(Action::Gizmo) < x(Action::GizmoOptions));
        assert!(x(Action::GizmoOptions) < x(Action::Overlays));
        assert!(x(Action::Overlays) < x(Action::Xray));
    }

    #[test]
    fn mode_dropdown_names_the_current_mode() {
        use crate::object_mode::ObjectMode;
        let user = UserDef::from_window_scale(1.0);
        let label = |m: ObjectMode| {
            let mut s = state();
            s.mode = m;
            layout(&user, header(), &s).into_iter().find(|i| i.action == Action::Mode).map(|i| match i.look {
                Look::Dropdown { label, icon } => (label, icon),
                _ => (None, Icon::ObjectMode),
            })
        };
        assert_eq!(label(ObjectMode::Edit), Some((Some("Edit Mode".to_string()), Icon::EditMode)));
        assert_eq!(label(ObjectMode::Sculpt), Some((Some("Sculpt Mode".to_string()), Icon::SculptMode)));
        assert_eq!(label(ObjectMode::Object), Some((Some("Object Mode".to_string()), Icon::ObjectMode)));
    }
}
