//! The Auto-Masking popover of the sculpt tool header: rows come from `sculpt_automask_layout::layout`
//! (`draw_mesh_automasking_settings(topbar=True)`), drawn as Blender checkbox and number-slider rows.
//! A click on a checkbox row toggles its flag, a press on a slider sets it from the position, a click outside closes.
//! Deviations: the cavity curve widget is not drawn; separators are half a row high.

use crate::area::{fill_rect, Rcti};
use crate::font;
use crate::sculpt_automask_layout::{self as rows, Row};
use crate::sculpt_automask_settings::AutomaskSettings;
use crate::theme::Rgba;
use crate::userdef::UserDef;

const OUTLINE: Rgba = Rgba::hex(0x242424ff);
const INNER: Rgba = Rgba::hex(0x181818ff);
const TITLE_TEXT: Rgba = Rgba::hex(0x999999ff);
const TEXT: Rgba = Rgba::hex(0xddddddff);
const TEXT_OFF: Rgba = Rgba::hex(0x777777ff);
const BOX: Rgba = Rgba::hex(0x545454ff);
const BOX_ON: Rgba = Rgba::hex(0x4772b3ff);
const SLIDER_INNER: Rgba = Rgba::hex(0x3d3d3dff);
const SLIDER_FILL: Rgba = Rgba::hex(0x5680c2ff);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Check,
    Slider,
}

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub id: &'static str,
    pub text: &'static str,
    pub kind: Kind,
    pub active: bool,
    pub rect: Rcti,
}

pub struct Layout {
    pub panel: Rcti,
    pub title: Rcti,
    pub items: Vec<Item>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Outside,
    Inside,
    Toggled,
    Slider,
}

/// `(min, max, is_int)` of a slider property (RNA ranges, see `sculpt_automask_settings.rs`).
fn slider_range(id: &str) -> Option<(f32, f32, bool)> {
    Some(match id {
        "boundary_edges_propagation_steps" => (1.0, 20.0, true),
        "cavity_blur_steps" => (0.0, 25.0, true),
        "cavity_factor" => (0.0, 5.0, false),
        "start_normal_limit" | "view_normal_limit" => (0.0001, std::f32::consts::PI, false),
        "start_normal_falloff" | "view_normal_falloff" => (0.0001, 1.0, false),
        _ => return None,
    })
}

pub fn slider_value(s: &AutomaskSettings, id: &str) -> f32 {
    match id {
        "boundary_edges_propagation_steps" => s.boundary_edges_propagation_steps() as f32,
        "cavity_blur_steps" => s.cavity_blur_steps() as f32,
        "cavity_factor" => s.cavity_factor(),
        "start_normal_limit" => s.start_normal_limit(),
        "start_normal_falloff" => s.start_normal_falloff(),
        "view_normal_limit" => s.view_normal_limit(),
        "view_normal_falloff" => s.view_normal_falloff(),
        _ => 0.0,
    }
}

pub fn set_slider(s: &mut AutomaskSettings, id: &str, v: f32) {
    match id {
        "boundary_edges_propagation_steps" => s.set_boundary_edges_propagation_steps(v.round() as i32),
        "cavity_blur_steps" => s.set_cavity_blur_steps(v.round() as i32),
        "cavity_factor" => s.set_cavity_factor(v),
        "start_normal_limit" => s.set_start_normal_limit(v),
        "start_normal_falloff" => s.set_start_normal_falloff(v),
        "view_normal_limit" => s.set_view_normal_limit(v),
        "view_normal_falloff" => s.set_view_normal_falloff(v),
        _ => {}
    }
}

pub fn flag_mut<'a>(s: &'a mut AutomaskSettings, id: &str) -> Option<&'a mut bool> {
    Some(match id {
        "use_automasking_topology" => &mut s.topology,
        "use_automasking_face_sets" => &mut s.face_sets,
        "use_automasking_boundary_edges" => &mut s.boundary_edges,
        "use_automasking_boundary_face_sets" => &mut s.boundary_face_sets,
        "use_automasking_cavity" => &mut s.cavity,
        "use_automasking_cavity_inverted" => &mut s.cavity_inverted,
        "use_automasking_custom_cavity_curve" => &mut s.custom_cavity_curve,
        "use_automasking_view_normal" => &mut s.view_normal,
        "use_automasking_view_occlusion" => &mut s.view_occlusion,
        "use_automasking_start_normal" => &mut s.start_normal,
        _ => return None,
    })
}

fn flag(s: &AutomaskSettings, id: &str) -> bool {
    flag_mut(&mut s.clone(), id).map(|f| *f).unwrap_or(false)
}

/// The panel opened at window position `pos` (top left, y up) in a `win_w` x `win_h` window.
pub fn layout(user: &UserDef, s: &AutomaskSettings, use_face_set: bool, pos: [i32; 2], win_w: i32, win_h: i32) -> Layout {
    let unit = user.widget_unit;
    let margin = (unit / 5).max(2);
    let width = 12 * unit + 2 * margin;
    let list = rows::layout(s, use_face_set, false);
    let mut height = 2 * margin;
    for r in &list {
        height += match r {
            Row::Separator => unit / 2,
            Row::CavityCurve => 0,
            _ => unit,
        };
    }
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
    let mut y = panel.ymax - margin;
    let mut row_rect = |h: i32| {
        let r = Rcti { xmin: panel.xmin + margin, xmax: panel.xmax - margin, ymax: y, ymin: y - h + 1 };
        y -= h;
        r
    };
    let mut title = row_rect(unit);
    let mut items = Vec::new();
    for (i, r) in list.iter().enumerate() {
        match *r {
            Row::Label(_) if i == 0 => title = title,
            Row::Label(_) | Row::CavityCurve => {}
            Row::Separator => {
                row_rect(unit / 2);
            }
            Row::Prop { id, text, active, .. } if i > 0 => {
                let kind = if id.starts_with("use_") { Kind::Check } else { Kind::Slider };
                let text = text.unwrap_or(if id == "boundary_edges_propagation_steps" { "Steps" } else { id });
                items.push(Item { id, text, kind, active, rect: row_rect(unit) });
            }
            _ => {}
        }
    }
    Layout { panel, title, items }
}

pub fn item_at(l: &Layout, xy: [i32; 2]) -> Option<&Item> {
    l.items.iter().find(|i| i.rect.isect_pt_v(xy))
}

fn fraction(rect: Rcti, x: i32) -> f32 {
    ((x - rect.xmin) as f32 / (rect.size_x().max(1) - 1).max(1) as f32).clamp(0.0, 1.0)
}

/// Sets the slider `item` from window x (press and drag).
pub fn drag_slider(item: &Item, x: i32, s: &mut AutomaskSettings) {
    if let Some((lo, hi, _)) = slider_range(item.id) {
        set_slider(s, item.id, lo + (hi - lo) * fraction(item.rect, x));
    }
}

/// A press at `xy`: toggles a checkbox, sets a slider, closes on a click outside the panel.
pub fn press(l: &Layout, xy: [i32; 2], s: &mut AutomaskSettings) -> Press {
    if !l.panel.isect_pt_v(xy) {
        return Press::Outside;
    }
    match item_at(l, xy) {
        Some(item) if item.kind == Kind::Check => {
            if let Some(f) = flag_mut(s, item.id) {
                *f = !*f;
            }
            Press::Toggled
        }
        Some(item) => {
            drag_slider(item, xy[0], s);
            Press::Slider
        }
        None => Press::Inside,
    }
}

fn value_text(id: &str, v: f32, is_int: bool) -> String {
    if is_int {
        format!("{}", v as i32)
    } else if id.ends_with("_limit") {
        format!("{:.0}\u{b0}", v.to_degrees())
    } else {
        format!("{v:.3}")
    }
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, l: &Layout, s: &AutomaskSettings) {
    let b = user.pixelsize.max(1);
    fill_rect(buf, width, height, l.panel, OUTLINE.to_xrgb());
    let inner = Rcti { xmin: l.panel.xmin + b, xmax: l.panel.xmax - b, ymin: l.panel.ymin + b, ymax: l.panel.ymax - b };
    fill_rect(buf, width, height, inner, INNER.to_xrgb());
    font::draw_label(buf, width, height, user, l.title, "Auto-Masking", TITLE_TEXT);
    for item in &l.items {
        let color = if item.active { TEXT } else { TEXT_OFF };
        match item.kind {
            Kind::Check => {
                let side = item.rect.size_y() * 3 / 5;
                let bx = Rcti { xmin: item.rect.xmin + b, xmax: item.rect.xmin + b + side - 1, ymin: item.rect.ymin + (item.rect.size_y() - side) / 2, ymax: item.rect.ymin + (item.rect.size_y() - side) / 2 + side - 1 };
                let on = flag(s, item.id);
                fill_rect(buf, width, height, bx, if on { BOX_ON } else { BOX }.to_xrgb());
                if on {
                    let m = (side / 4).max(1);
                    fill_rect(buf, width, height, Rcti { xmin: bx.xmin + m, xmax: bx.xmax - m, ymin: bx.ymin + m, ymax: bx.ymax - m }, TEXT.to_xrgb());
                }
                let label = Rcti { xmin: bx.xmax + 1 + side / 2, ..item.rect };
                font::draw_label(buf, width, height, user, label, item.text, color);
            }
            Kind::Slider => {
                let (lo, hi, is_int) = slider_range(item.id).unwrap_or((0.0, 1.0, false));
                let v = slider_value(s, item.id);
                fill_rect(buf, width, height, item.rect, SLIDER_INNER.to_xrgb());
                let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
                let w = (t * item.rect.size_x() as f32) as i32;
                if w > 0 {
                    fill_rect(buf, width, height, Rcti { xmax: item.rect.xmin + w - 1, ..item.rect }, SLIDER_FILL.to_xrgb());
                }
                font::draw_label(buf, width, height, user, item.rect, &format!("{}: {}", item.text, value_text(item.id, v, is_int)), color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserDef {
        UserDef::from_window_scale(1.0)
    }

    fn open(s: &AutomaskSettings) -> Layout {
        layout(&user(), s, true, [100, 700], 1200, 800)
    }

    fn center(r: Rcti) -> [i32; 2] {
        [(r.xmin + r.xmax) / 2, (r.ymin + r.ymax) / 2]
    }

    #[test]
    fn a_click_on_a_checkbox_row_toggles_its_flag() {
        let mut s = AutomaskSettings::default();
        let l = open(&s);
        let topo = l.items.iter().find(|i| i.id == "use_automasking_topology").unwrap();
        assert_eq!(press(&l, center(topo.rect), &mut s), Press::Toggled);
        assert!(s.topology);
        assert_eq!(press(&l, center(topo.rect), &mut s), Press::Toggled);
        assert!(!s.topology);
    }

    #[test]
    fn a_click_outside_closes() {
        let mut s = AutomaskSettings::default();
        let l = open(&s);
        assert_eq!(press(&l, [l.panel.xmax + 20, l.panel.ymin - 20], &mut s), Press::Outside);
        assert_eq!(s, AutomaskSettings::default());
    }

    #[test]
    fn rows_do_not_overlap_and_stay_in_the_panel() {
        let mut s = AutomaskSettings::default();
        s.boundary_edges = true;
        s.cavity = true;
        s.view_normal = true;
        s.start_normal = true;
        let l = open(&s);
        for (i, a) in l.items.iter().enumerate() {
            assert!(a.rect.xmin >= l.panel.xmin && a.rect.xmax <= l.panel.xmax && a.rect.ymin >= l.panel.ymin && a.rect.ymax <= l.panel.ymax);
            for b in &l.items[i + 1..] {
                let apart = a.rect.ymin > b.rect.ymax || b.rect.ymin > a.rect.ymax;
                assert!(apart, "{} overlaps {}", a.id, b.id);
            }
        }
    }

    #[test]
    fn opening_a_block_adds_its_sliders() {
        let mut s = AutomaskSettings::default();
        assert!(open(&s).items.iter().all(|i| i.kind == Kind::Check));
        s.start_normal = true;
        assert!(open(&s).items.iter().any(|i| i.id == "start_normal_limit" && i.kind == Kind::Slider));
    }

    #[test]
    fn a_slider_press_maps_position_to_value() {
        let mut s = AutomaskSettings::default();
        s.cavity = true;
        let l = open(&s);
        let it = *l.items.iter().find(|i| i.id == "cavity_factor").unwrap();
        press(&l, [it.rect.xmax, center(it.rect)[1]], &mut s);
        assert_eq!(s.cavity_factor(), 5.0);
        drag_slider(&it, it.rect.xmin, &mut s);
        assert_eq!(s.cavity_factor(), 0.0);
        let mut steps = AutomaskSettings::default();
        steps.boundary_edges = true;
        let l = open(&steps);
        let it = *l.items.iter().find(|i| i.id == "boundary_edges_propagation_steps").unwrap();
        let mut s2 = steps.clone();
        drag_slider(&it, it.rect.xmax, &mut s2);
        assert_eq!(s2.boundary_edges_propagation_steps(), 20);
    }

    #[test]
    fn the_panel_moves_back_into_the_window() {
        let s = AutomaskSettings::default();
        let l = layout(&user(), &s, true, [1190, 5], 1200, 800);
        assert!(l.panel.xmax <= 1199 && l.panel.ymin >= 0);
    }

    #[test]
    fn drawing_changes_pixels() {
        let mut s = AutomaskSettings::default();
        s.topology = true;
        let l = open(&s);
        let mut buf = vec![0u32; 1200 * 800];
        draw(&mut buf, 1200, 800, &user(), &l, &s);
        assert!(buf.iter().any(|&p| p != 0));
    }
}
