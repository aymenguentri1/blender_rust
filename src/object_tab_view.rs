//! The Object tab of the properties editor as Blender draws it (`OBJECT_PT_transform` with `use_property_split`,
//! properties_object.py:36-85): panel frames (`panel_frame`), right aligned labels (`Location X`, `Y`, `Z`), number
//! fields of an aligned column, the `lock_*` decorator and the animate dot after each row, the rotation Mode dropdown
//! with its convert menu, the `Delta Transform` sub-panel and the panels below it. A click on a header left of the grip
//! toggles `PNL_CLOSED` (`ui_handle_panel_header`, interface_panel.cc:2355); Ctrl + click on a top-level panel collapses
//! the others (`panels_collapse_all`). Panels whose content is not ported open to an empty body.
//! Offsets are those of Blender at UI scale 1 (measured on a 5.4 screenshot); everything scales with `scale_factor`.

use crate::area::Rcti;
use crate::modifier_panel::{Style, Widget};
use crate::object_tab_layout::PANELS;
use crate::panel_frame;
use crate::scene::{Scene, TransformProp};
use crate::transform_panel::{number, value, Edit, Field, Layout};
use crate::userdef::UserDef;

/// Panels drawn collapsed after Transform, in the order Blender shows them for a mesh.
pub const COLLAPSED: [&str; 10] = [
    "Relations", "Collections", "Instancing", "Motion Paths", "Shading", "Visibility", "Viewport Display", "Line Art", "Animation",
    "Custom Properties",
];

/// Every panel of the tab with its open state, in drawing order.
pub const ALL: [&str; 12] = [
    "Transform", "Delta Transform", "Relations", "Collections", "Instancing", "Motion Paths", "Shading", "Visibility", "Viewport Display",
    "Line Art", "Animation", "Custom Properties",
];

/// `PNL_CLOSED` of each panel, stored as the panels toggled away from their default (only Transform starts open).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PanelOpen {
    toggled: u16,
}

impl PanelOpen {
    fn bit(label: &str) -> u16 {
        ALL.iter().position(|l| *l == label).map_or(0, |i| 1 << i)
    }

    pub fn is_open(&self, label: &str) -> bool {
        let default_open = label == "Transform";
        default_open ^ (self.toggled & Self::bit(label) != 0)
    }

    /// A click on the header of `label`; with `ctrl` on a top-level panel the others are closed and it opens.
    pub fn click(&mut self, label: &str, ctrl: bool) {
        if ctrl && label != "Delta Transform" {
            let delta_open = self.is_open("Delta Transform");
            self.toggled = 0;
            for l in ALL {
                if l != "Delta Transform" && self.is_open(l) != (l == label) {
                    self.toggled ^= Self::bit(l);
                }
            }
            if delta_open {
                self.toggled ^= Self::bit("Delta Transform");
            }
            return;
        }
        self.toggled ^= Self::bit(label);
    }
}

/// Whether `pt` is in the part of the header that toggles it: left of the grip (`PNL_ICON * 1.5`).
pub fn header_at(layout: &Layout, user: &UserDef, pt: [i32; 2]) -> Option<&'static str> {
    let grip = (user.widget_unit as f32 * 1.5) as i32;
    layout.headers.iter().find(|(r, _)| r.isect_pt_v(pt) && pt[0] < r.xmax - grip).map(|(_, l)| *l)
}

/// Vertical centres of the rows below the top of the Transform body, at scale 1: Location (3), Rotation (3), Mode, Scale (3) and the Delta Transform header.
const ROW_Y: [f32; 11] = [17.5, 37.0, 56.5, 81.5, 101.5, 121.5, 146.5, 171.5, 191.5, 211.5, 243.5];
/// Height of the Transform body at scale 1 (to the bottom of the collapsed Delta Transform header).
const BODY_H: f32 = 257.0;
/// Gap between two panels at scale 1.
const PANEL_GAP: f32 = 2.0;

fn widget(rect: Rcti, label: &str, style: Style) -> Widget {
    Widget { rect, label: label.into(), style, action: None, centered: false }
}

/// A panel whose content is not ported: its header, and an empty body when open.
fn plain_panel(user: &UserDef, out: &mut Layout, x0: i32, x1: i32, top: i32, label: &'static str, open: bool) -> i32 {
    let hh = panel_frame::header_height(user);
    let body_h = if open { (8.0 * user.scale_factor as f32) as i32 } else { 0 };
    let body = Rcti { xmin: x0, xmax: x1, ymin: top - hh - body_h, ymax: top - hh };
    let frame = panel_frame::frame(user, body, open, false);
    out.widgets.push(widget(body, "", Style::Frame { open, active: false }));
    out.widgets.push(widget(frame.title, label, Style::Title));
    out.widgets.push(widget(frame.header_rect, "", Style::PanelArrow { open }));
    out.headers.push((frame.header_rect, label));
    body.ymin - (PANEL_GAP * user.scale_factor as f32) as i32
}

/// A collapsed-list panel: Relations draws its rows when open (`object_tab_relations`), the others an empty body.
fn list_panel(user: &UserDef, out: &mut Layout, x: (i32, i32), top: i32, label: &'static str, open: bool, scene: &Scene, o: &crate::scene::Object) -> i32 {
    let g = match label {
        "Relations" => crate::object_tab_relations::groups(scene, o).to_vec(),
        "Visibility" => crate::object_tab_visibility::groups(o),
        _ => Vec::new(),
    };
    if !open || g.is_empty() {
        return plain_panel(user, out, x.0, x.1, top, label, open);
    }
    let hh = panel_frame::header_height(user);
    let (mx, _) = panel_frame::margins(user);
    let body = Rcti { xmin: x.0, xmax: x.1, ymin: top - hh - crate::object_tab_relations::body_height(user, &g), ymax: top - hh };
    let frame = panel_frame::frame(user, body, true, false);
    out.widgets.push(widget(body, "", Style::Frame { open: true, active: false }));
    out.widgets.push(widget(frame.title, label, Style::Title));
    out.widgets.push(widget(frame.header_rect, "", Style::PanelArrow { open: true }));
    out.headers.push((frame.header_rect, label));
    out.widgets.extend(crate::object_tab_relations::widgets(user, &g, x.0 + mx, x.1 - mx, body.ymax));
    body.ymin - (PANEL_GAP * user.scale_factor as f32) as i32
}

/// The tab in `window` (window coordinates, y up). Same output as `transform_panel::layout`.
pub fn layout(user: &UserDef, window: Rcti, scene: &Scene, edit: Option<&Edit>, open: PanelOpen) -> Layout {
    let s = user.scale_factor as f32;
    let (mx, _) = panel_frame::margins(user);
    let pad = (user.widget_unit as f32 * 0.4) as i32;
    let (x0, x1) = (window.xmin + pad, window.xmax - pad);
    let mut out = Layout { widgets: Vec::new(), fields: Vec::new(), bottom: window.ymax, headers: Vec::new() };
    let Some(o) = scene.active_object().filter(|o| !o.hidden) else { return out };
    let hh = panel_frame::header_height(user);
    let sc = |v: f32| (v * s).round() as i32;
    /* The context path and the ID row (object_tab_context.rs) sit above the first panel. */
    let top = crate::object_tab_context::layout(user, window).map_or(window.ymax - pad, |r| r.panels_top);
    let body_top = top - hh;
    if !open.is_open("Transform") {
        let mut y = plain_panel(user, &mut out, x0, x1, top, PANELS[0].label, false);
        for label in COLLAPSED {
            y = list_panel(user, &mut out, (x0, x1), y, label, open.is_open(label), scene, o);
        }
        out.bottom = y;
        return out;
    }
    let body = Rcti { xmin: x0, xmax: x1, ymin: body_top - sc(BODY_H), ymax: body_top };
    let frame = panel_frame::frame(user, body, true, false);
    out.widgets.push(widget(body, "", Style::Frame { open: true, active: false }));
    out.widgets.push(widget(frame.title, PANELS[0].label, Style::Title));
    out.widgets.push(widget(frame.header_rect, "", Style::PanelArrow { open: true }));
    out.headers.push((frame.header_rect, PANELS[0].label));

    /* Property split: labels end at 40 % of the row, then the field, then two decorator cells. */
    let (ix0, ix1) = (x0 + mx, x1 - mx);
    let cell = user.widget_unit;
    let fx0 = ix0 + ((ix1 - ix0) as f32 * 0.4).round() as i32;
    let fx1 = ix1 - 2 * cell + 1;
    let row_rect = |i: usize, xa: i32, xb: i32| {
        let c = body_top - sc(ROW_Y[i]);
        Rcti { xmin: xa, xmax: xb, ymin: c - cell / 2, ymax: c - cell / 2 + cell - 1 }
    };
    let groups = [(TransformProp::Location, "Location", 0usize), (TransformProp::Rotation, "Rotation", 3), (TransformProp::Scale, "Scale", 7)];
    for (prop, name, first) in groups {
        for axis in 0..3 {
            let i = first + axis;
            let r = row_rect(i, fx0, fx1);
            let label = if axis == 0 { format!("{name} X") } else { ["", "Y", "Z"][axis].to_string() };
            out.widgets.push(widget(row_rect(i, ix0, fx0 - 1), &label, Style::PropLabel));
            let field = Field { prop, axis };
            let editing = edit.filter(|e| e.field == field);
            let v = value(scene, field).unwrap_or(0.0);
            let text = match (editing, prop) {
                (Some(e), _) => format!("{}|", e.text),
                (None, TransformProp::Scale) => format!("{v:.3}"),
                (None, TransformProp::Rotation) => format!("{}\u{b0}", number(v)),
                (None, _) => format!("{} m", number(v)),
            };
            let corners = [1u8, 2, 3][axis];
            let style = if editing.is_some() { Style::Editing } else { Style::PropBox { corners } };
            out.widgets.push(widget(r, &text, style));
            out.fields.push((r, field));
            out.widgets.push(widget(row_rect(i, fx1 + 1, fx1 + cell), "", Style::Lock { locked: false }));
            out.widgets.push(widget(row_rect(i, fx1 + cell + 1, ix1), "", Style::KeyDot));
        }
    }
    /* Mode row. */
    out.widgets.push(widget(row_rect(6, ix0, fx0 - 1), "Mode", Style::PropLabel));
    out.widgets.push(widget(row_rect(6, fx0, fx1), "XYZ Euler", Style::DropBox));
    out.widgets.push(widget(row_rect(6, fx1 + 1, fx1 + cell), "", Style::MenuIcon));
    /* Delta Transform: a collapsed sub-panel header inside the body. */
    let dc = body_top - sc(ROW_Y[10]);
    let arrow = Rcti { xmin: ix0 + sc(6.0), xmax: ix0 + sc(6.0) + cell - 1, ymin: dc - cell / 2, ymax: dc - cell / 2 + cell - 1 };
    out.widgets.push(widget(arrow, "", Style::DisclosureClosed));
    out.widgets.push(widget(Rcti { xmin: arrow.xmax + 1, xmax: ix1, ..arrow }, PANELS[1].label, Style::Title));

    let mut y = body.ymin - sc(PANEL_GAP);
    for label in COLLAPSED {
        y = list_panel(user, &mut out, (x0, x1), y, label, open.is_open(label), scene, o);
    }
    out.bottom = y;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_names_are_the_ones_blender_lists() {
        assert_eq!(COLLAPSED[0], "Relations");
        assert_eq!(COLLAPSED.len(), 10);
        assert_eq!(PANELS[0].label, "Transform");
    }

    fn lay(open: PanelOpen) -> (UserDef, Layout) {
        let user = UserDef::from_window_scale(1.0);
        let mut s = Scene::startup();
        s.active = s.objects.iter().find(|o| o.name == "Cube").map(|o| o.id);
        let l = layout(&user, Rcti { xmin: 28, xmax: 337, ymin: 0, ymax: 757 }, &s, None, open);
        (user, l)
    }

    fn header(l: &Layout, label: &str) -> Rcti {
        l.headers.iter().find(|(_, h)| *h == label).unwrap().0
    }

    #[test]
    fn only_transform_starts_open() {
        let o = PanelOpen::default();
        assert!(o.is_open("Transform"));
        assert!(ALL[1..].iter().all(|l| !o.is_open(l)));
        let (_, l) = lay(o);
        assert_eq!(l.headers.len(), 11, "Transform and the ten panels below it");
    }

    #[test]
    fn a_header_click_left_of_the_grip_toggles_the_panel() {
        let (user, l) = lay(PanelOpen::default());
        let r = header(&l, "Relations");
        let mid = (r.ymin + r.ymax) / 2;
        assert_eq!(header_at(&l, &user, [r.xmin + 30, mid]), Some("Relations"));
        assert_eq!(header_at(&l, &user, [r.xmax - 5, mid]), None, "the grip drags, it does not collapse");
        let mut o = PanelOpen::default();
        o.click("Relations", false);
        assert!(o.is_open("Relations"));
        let (_, l2) = lay(o);
        /* The open panel pushes the ones below it down. */
        assert!(header(&l2, "Collections").ymax < header(&l, "Collections").ymax);
        o.click("Relations", false);
        assert_eq!(o, PanelOpen::default());
    }

    #[test]
    fn closing_transform_leaves_only_headers() {
        let mut o = PanelOpen::default();
        o.click("Transform", false);
        let (_, l) = lay(o);
        assert!(l.fields.is_empty());
        let (_, open) = lay(PanelOpen::default());
        assert!(header(&l, "Relations").ymax > header(&open, "Relations").ymax);
    }

    #[test]
    fn ctrl_click_collapses_all_others() {
        let mut o = PanelOpen::default();
        o.click("Visibility", false);
        o.click("Collections", true);
        assert!(o.is_open("Collections"));
        assert!(!o.is_open("Transform") && !o.is_open("Visibility"));
    }

    #[test]
    fn row_offsets_increase_and_fit_the_body() {
        assert!(ROW_Y.windows(2).all(|w| w[0] < w[1]));
        assert!(ROW_Y[10] + 12.5 < BODY_H);
    }
}
