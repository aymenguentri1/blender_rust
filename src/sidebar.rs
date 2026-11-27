//! The sidebar region of the 3D viewport (`RGN_TYPE_UI`, space_view3d.cc `view3d_buttons_register`) with the Item tab
//! and its Transform panel (`view3d_panel_transform`, view3d_buttons.cc): `Location:`, `Rotation:`, `Scale:` and
//! `Dimensions:` blocks of three aligned fields (label `X`/`Y`/`Z` left, value right), the lock decorators, the
//! rotation mode dropdown with its convert menu, and the category tab strip (Item, Tool, View, Animation) with the
//! text rotated by 90 degrees. Offsets are those of Blender at UI scale 1 (measured on a 5.4 screenshot).

use crate::area::{view3d_sidebar_rect, Rcti};
use crate::font;
use crate::modifier_panel::{Style, Widget};
use crate::panel_frame;
use crate::scene::{Scene, TransformProp};
use crate::theme::Rgba;
use crate::transform_panel::{number, value, Edit, Field, Layout};
use crate::userdef::UserDef;

pub const TABS: [&str; 4] = ["Item", "Tool", "View", "Animation"];
const TAB_W: f32 = 22.0;
const LABEL_Y: [f32; 4] = [41.0, 123.0, 227.0, 309.0];
const ROW_Y: [f32; 13] = [61.0, 80.5, 100.0, 143.0, 162.0, 182.0, 205.0, 247.0, 266.0, 286.0, 329.0, 348.0, 368.0];
const BODY_H: f32 = 382.0;

fn widget(rect: Rcti, label: &str, style: Style) -> Widget {
    Widget { rect, label: label.into(), style, action: None, centered: false }
}

/// The Transform panel in the sidebar `region`.
pub fn layout(user: &UserDef, region: Rcti, scene: &Scene, edit: Option<&Edit>) -> Layout {
    let s = user.scale_factor as f32;
    let sc = |v: f32| (v * s).round() as i32;
    let mut out = Layout { widgets: Vec::new(), fields: Vec::new(), bottom: region.ymin };
    let Some(o) = scene.active_object().filter(|o| !o.hidden) else { return out };
    let (px0, px1) = (region.xmin + sc(2.0), region.xmax - sc(TAB_W + 6.0));
    let top = region.ymax - sc(4.0);
    let hh = panel_frame::header_height(user);
    let all = Rcti { xmin: px0, xmax: px1, ymin: top - sc(BODY_H), ymax: top };
    out.widgets.push(widget(all, "", Style::SideFrame));
    let header = Rcti { ymin: top - hh, ..all };
    let frame = panel_frame::frame(user, Rcti { ymax: header.ymin, ymin: header.ymin, ..header }, true, false);
    out.widgets.push(widget(frame.title, "Transform", Style::Title));
    out.widgets.push(widget(header, "", Style::PanelArrow { open: true }));
    let (fx0, cell) = (px0 + sc(6.0), user.widget_unit - 1);
    let (fx1_lock, fx1_free) = (px1 - sc(61.0), px1 - sc(9.0));
    let row = |i: usize, xa: i32, xb: i32| {
        let c = top - sc(ROW_Y[i]);
        Rcti { xmin: xa, xmax: xb, ymin: c - cell / 2, ymax: c - cell / 2 + cell - 1 }
    };
    let groups = [(TransformProp::Location, "Location:", 0usize, true), (TransformProp::Rotation, "Rotation:", 3, true), (TransformProp::Scale, "Scale:", 7, true), (TransformProp::Dimensions, "Dimensions:", 10, false)];
    for (g, (prop, name, first, lock)) in groups.into_iter().enumerate() {
        if prop == TransformProp::Dimensions && o.dimensions().is_none() {
            continue;
        }
        let lc = top - sc(LABEL_Y[g]);
        out.widgets.push(widget(Rcti { xmin: fx0, xmax: px1, ymin: lc - cell / 2, ymax: lc + cell / 2 }, name, Style::Title));
        for axis in 0..3 {
            let r = row(first + axis, fx0, if lock { fx1_lock } else { fx1_free });
            let field = Field { prop, axis };
            let editing = edit.filter(|e| e.field == field);
            let v = value(scene, field).unwrap_or(0.0);
            let text = match (editing, prop) {
                (Some(e), _) => format!("{}|", e.text),
                (None, TransformProp::Scale) => format!("{v:.3}"),
                (None, TransformProp::Rotation) => format!("{}\u{b0}", number(v)),
                (None, _) => format!("{} m", number(v)),
            };
            let label = format!("{}\t{}", ["X", "Y", "Z"][axis], text);
            let style = if editing.is_some() { Style::Editing } else { Style::LabeledBox { corners: [1u8, 2, 3][axis] } };
            out.widgets.push(widget(r, if editing.is_some() { &text } else { &label }, style));
            out.fields.push((r, field));
            if lock {
                let cx = fx1_lock + sc(33.0);
                out.widgets.push(widget(Rcti { xmin: cx - cell / 2, xmax: cx + cell / 2, ..r }, "", Style::Lock { locked: false }));
            }
        }
        if prop == TransformProp::Rotation {
            out.widgets.push(widget(row(6, fx0, px1 - sc(28.0)), "XYZ Euler", Style::DropBox));
            out.widgets.push(widget(row(6, px1 - sc(28.0) + sc(4.0), px1 - sc(6.0)), "", Style::MenuIcon));
        }
    }
    out.bottom = all.ymin;
    out
}

/// The category tab strip at the right edge of `region`: `(rect, name, active)` for each tab.
pub fn tabs(user: &UserDef, region: Rcti) -> Vec<(Rcti, &'static str, bool)> {
    let s = user.scale_factor as f32;
    let sc = |v: f32| (v * s).round() as i32;
    let mut y = region.ymax - sc(4.0 + 3.0);
    let mut out = Vec::new();
    for (i, name) in TABS.iter().enumerate() {
        let h = font::string_width(user, name) + sc(16.0);
        out.push((Rcti { xmin: region.xmax - sc(TAB_W) + 1, xmax: region.xmax, ymin: y - h + 1, ymax: y }, *name, i == 0));
        y -= h + sc(8.0);
    }
    out
}

/// Draws the tab strip; the text reads from bottom to top.
pub fn draw_tabs(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti) {
    for (rect, name, active) in tabs(user, region) {
        let bg = if active { Rgba::hex(0x3d3d3dff) } else { Rgba::hex(0x2b2b2bff) };
        crate::widget_draw::draw_round_box(buf, width, height, user, rect, bg, Rgba::hex(0), 0.3);
        let (tw, th) = ((rect.size_y() + 1) as usize, (rect.size_x() + 1) as usize);
        let px = (bg.r as u32) << 16 | (bg.g as u32) << 8 | bg.b as u32;
        let mut tmp = vec![px; tw * th];
        let text_w = font::string_width(user, name);
        font::draw_label_pad(&mut tmp, tw, th, user, Rcti { xmin: 0, xmax: tw as i32 - 1, ymin: 0, ymax: th as i32 - 1 }, name, Rgba::hex(0xd9d9d9ff), (tw as i32 - text_w) / 2);
        let (left, top_row) = (rect.xmin, height as i32 - 1 - rect.ymax);
        for r in 0..th {
            for c in 0..tw {
                let (x, y) = (left + r as i32, top_row + (tw - 1 - c) as i32);
                if x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height {
                    buf[y as usize * width + x as usize] = tmp[r * tw + c];
                }
            }
        }
    }
}

/// The sidebar region of the 3D viewport area `main`.
pub fn region(user: &UserDef, main: Rcti) -> Option<Rcti> {
    view3d_sidebar_rect(user, main)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_width_and_tabs_at_1x_and_2x() {
        for (scale, w) in [(1.0_f64, 270), (2.0, 540)] {
            let user = UserDef::from_window_scale(scale);
            let main = Rcti { xmin: 0, xmax: 2999, ymin: 0, ymax: 1799 };
            let r = view3d_sidebar_rect(&user, main).unwrap();
            assert_eq!(r.xmax - r.xmin + 1, w);
            let t = tabs(&user, r);
            assert_eq!(t.len(), 4);
            assert!(t.windows(2).all(|p| p[1].0.ymax < p[0].0.ymin));
            assert!(t.iter().all(|(rect, _, _)| rect.xmax == r.xmax));
        }
    }

    #[test]
    fn rows_are_ordered_and_fit_the_panel() {
        assert!(ROW_Y.windows(2).all(|w| w[0] < w[1]));
        assert!(ROW_Y[12] + 10.0 <= BODY_H && LABEL_Y.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn narrow_areas_have_no_sidebar() {
        let user = UserDef::from_window_scale(1.0);
        assert!(view3d_sidebar_rect(&user, Rcti { xmin: 0, xmax: 700, ymin: 0, ymax: 500 }).is_none());
    }
}
