//! The Object tab of the properties editor as Blender draws it (`OBJECT_PT_transform` with `use_property_split`,
//! properties_object.py:36-85): panel frames (`panel_frame`), right aligned labels (`Location X`, `Y`, `Z`), number
//! fields of an aligned column, the `lock_*` decorator and the animate dot after each row, the rotation Mode dropdown
//! with its convert menu, the collapsed `Delta Transform` sub-panel and the collapsed panels below it.
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

/// Vertical centres of the rows below the top of the Transform body, at scale 1: Location (3), Rotation (3), Mode, Scale (3) and the Delta Transform header.
const ROW_Y: [f32; 11] = [17.5, 37.0, 56.5, 81.5, 101.5, 121.5, 146.5, 171.5, 191.5, 211.5, 243.5];
/// Height of the Transform body at scale 1 (to the bottom of the collapsed Delta Transform header).
const BODY_H: f32 = 257.0;
/// Gap between two panels at scale 1.
const PANEL_GAP: f32 = 2.0;

fn widget(rect: Rcti, label: &str, style: Style) -> Widget {
    Widget { rect, label: label.into(), style, action: None, centered: false }
}

fn collapsed_panel(user: &UserDef, out: &mut Vec<Widget>, x0: i32, x1: i32, top: i32, label: &str) -> i32 {
    let hh = panel_frame::header_height(user);
    let body = Rcti { xmin: x0, xmax: x1, ymin: top - hh, ymax: top - hh };
    let frame = panel_frame::frame(user, body, false, false);
    out.push(widget(body, "", Style::Frame { open: false, active: false }));
    out.push(widget(frame.title, label, Style::Title));
    out.push(widget(frame.header_rect, "", Style::PanelArrow { open: false }));
    top - hh - (PANEL_GAP * user.scale_factor as f32) as i32
}

/// The tab in `window` (window coordinates, y up). Same output as `transform_panel::layout`.
pub fn layout(user: &UserDef, window: Rcti, scene: &Scene, edit: Option<&Edit>) -> Layout {
    let s = user.scale_factor as f32;
    let (mx, _) = panel_frame::margins(user);
    let pad = (user.widget_unit as f32 * 0.4) as i32;
    let (x0, x1) = (window.xmin + pad, window.xmax - pad);
    let mut out = Layout { widgets: Vec::new(), fields: Vec::new(), bottom: window.ymax };
    let Some(o) = scene.active_object().filter(|o| !o.hidden) else { return out };
    let _ = o;
    let hh = panel_frame::header_height(user);
    let sc = |v: f32| (v * s).round() as i32;
    /* The context path and the ID row (object_tab_context.rs) sit above the first panel. */
    let top = crate::object_tab_context::layout(user, window).map_or(window.ymax - pad, |r| r.panels_top);
    let body_top = top - hh;
    let body = Rcti { xmin: x0, xmax: x1, ymin: body_top - sc(BODY_H), ymax: body_top };
    let frame = panel_frame::frame(user, body, true, false);
    out.widgets.push(widget(body, "", Style::Frame { open: true, active: false }));
    out.widgets.push(widget(frame.title, PANELS[0].label, Style::Title));
    out.widgets.push(widget(frame.header_rect, "", Style::PanelArrow { open: true }));

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
        y = collapsed_panel(user, &mut out.widgets, x0, x1, y, label);
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

    #[test]
    fn row_offsets_increase_and_fit_the_body() {
        assert!(ROW_Y.windows(2).all(|w| w[0] < w[1]));
        assert!(ROW_Y[10] + 12.5 < BODY_H);
    }
}
