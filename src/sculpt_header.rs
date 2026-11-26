//! The tool settings header of the 3D viewport in Sculpt Mode (`VIEW3D_HT_tool_header`, the "tool header"
//! region under the main header): the brush dropdown, the Radius and Strength sliders, the Add / Subtract
//! direction and the X / Y / Z symmetry toggles.
//!
//! Layout, drawing and hit testing live here; `main.rs` routes the mouse. A press on a slider sets its value from
//! the position inside it and the drag goes on setting it (Blender's number slider drags relative to the press,
//! this one is absolute). The brush dropdown has no popup of brushes: a press selects the next brush, Shift the
//! previous one.

use crate::area::{global_header_layout_start, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::sculpt::{Brush, Settings, MAX_RADIUS_PX, MIN_RADIUS_PX};
use crate::theme::{self, Rgba};
use crate::userdef::UserDef;
use crate::widget_draw::{draw_round_box, draw_round_box_sides};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Brush,
    Radius,
    Strength,
    Add,
    Subtract,
    Symmetry(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub action: Action,
    pub rect: Rcti,
    left: bool,
    right: bool,
}

/// `wcol_numslider`: the filled part of a slider.
const SLIDER_FILL: Rgba = Rgba::hex(0x5680c2ff);
const SLIDER_INNER: Rgba = Rgba::hex(0x3d3d3dff);
const TEXT: Rgba = Rgba::hex(0xe6e6e6ff);

fn unit(user: &UserDef) -> i32 {
    user.widget_unit
}

/// Lays out the buttons for the tool header region `header` (window coordinates).
pub fn layout(user: &UserDef, header: Rcti) -> Vec<Item> {
    let (offset, top_y, height) = global_header_layout_start(user, header.size_y() + 1);
    let y0 = header.ymin + top_y - height;
    let y1 = header.ymin + top_y - 1;
    let u = unit(user);
    let space = user.button_space_x;
    let mut x = header.xmin + offset;
    let mut items = Vec::new();
    let mut put = |action: Action, width: i32, gap: bool, left: bool, right: bool, x: &mut i32| {
        items.push(Item { action, rect: Rcti { xmin: *x, xmax: *x + width - 1, ymin: y0, ymax: y1 }, left, right });
        *x += width + if gap { space } else { 0 };
    };
    /* The brush dropdown is as wide as its longest name, so that the buttons after it do not jump around. */
    let longest = Brush::ALL.iter().map(|b| font::string_width(user, b.label())).max().unwrap_or(0);
    put(Action::Brush, u + longest + (user.scale_factor * 4.0) as i32 + u * 3 / 4, true, false, false, &mut x);
    put(Action::Radius, u * 7, true, false, false, &mut x);
    put(Action::Strength, u * 7, true, false, false, &mut x);
    let add_w = font::label_width(user, "Add");
    let sub_w = font::label_width(user, "Subtract");
    put(Action::Add, add_w, false, false, true, &mut x);
    put(Action::Subtract, sub_w, true, true, false, &mut x);
    for axis in 0..3 {
        put(Action::Symmetry(axis), u, false, axis > 0, axis < 2, &mut x);
    }
    items.retain(|i| i.rect.xmax <= header.xmax);
    items
}

pub fn action_at(items: &[Item], xy: [i32; 2]) -> Option<Action> {
    items.iter().find(|i| i.rect.isect_pt_v(xy)).map(|i| i.action)
}

pub fn rect_of(items: &[Item], action: Action) -> Option<Rcti> {
    items.iter().find(|i| i.action == action).map(|i| i.rect)
}

/// How far across a slider `x` is (0 at the left edge, 1 at the right edge).
fn fraction(rect: Rcti, x: i32) -> f32 {
    ((x - rect.xmin) as f32 / (rect.size_x().max(1)) as f32).clamp(0.0, 1.0)
}

/// What a slider shows as its filled part, 0 to 1.
pub fn slider_fraction(action: Action, s: &Settings) -> f32 {
    match action {
        Action::Radius => ((s.radius_px - MIN_RADIUS_PX) / (MAX_RADIUS_PX - MIN_RADIUS_PX)).clamp(0.0, 1.0),
        Action::Strength => s.strength.clamp(0.0, 1.0),
        _ => 0.0,
    }
}

/// A press or drag at window x `x` on the slider `action` (`rect` is its rect). Returns whether a value changed.
pub fn set_slider(action: Action, rect: Rcti, x: i32, s: &mut Settings) -> bool {
    let f = fraction(rect, x);
    match action {
        Action::Radius => {
            s.radius_px = (MIN_RADIUS_PX + f * (MAX_RADIUS_PX - MIN_RADIUS_PX)).round();
            true
        }
        Action::Strength => {
            s.strength = (f * 100.0).round() / 100.0;
            true
        }
        _ => false,
    }
}

/// The brush after (or, `back`, before) `current` in `Brush::ALL`.
pub fn cycle_brush(current: Brush, back: bool) -> Brush {
    let n = Brush::ALL.len();
    let i = Brush::ALL.iter().position(|&b| b == current).unwrap_or(0);
    Brush::ALL[if back { (i + n - 1) % n } else { (i + 1) % n }]
}

/// A press on a button that is not a slider: selects the next brush, sets the direction or toggles a symmetry axis.
/// Returns whether anything changed.
pub fn press(action: Action, shift: bool, s: &mut Settings) -> bool {
    match action {
        Action::Brush => {
            s.brush = cycle_brush(s.brush, shift);
            true
        }
        Action::Add => std::mem::replace(&mut s.subtract, false),
        Action::Subtract => !std::mem::replace(&mut s.subtract, true),
        Action::Symmetry(axis) => {
            s.symmetry[axis] = !s.symmetry[axis];
            true
        }
        Action::Radius | Action::Strength => false,
    }
}

fn lighten(c: Rgba, by: u8) -> Rgba {
    Rgba { r: c.r.saturating_add(by), g: c.g.saturating_add(by), b: c.b.saturating_add(by), a: c.a }
}

/// Draws the tool header buttons for the settings `s`; `hover` is the action under the cursor.
pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, items: &[Item], s: &Settings, hover: Option<Action>) {
    let u = unit(user);
    for item in items {
        let hot = hover == Some(item.action);
        let r = item.rect;
        let tool = if hot { lighten(theme::WCOL_TOOL_INNER, 16) } else { theme::WCOL_TOOL_INNER };
        let outline = theme::WCOL_TOOL_OUTLINE;
        let round = theme::WCOL_TOOL_ROUNDNESS;
        match item.action {
            Action::Brush => {
                draw_round_box(buf, width, height, user, r, tool, outline, round);
                icons::draw_icon(buf, width, height, user, Rcti { xmax: r.xmin + u - 1, ..r }, Icon::SculptMode, TEXT);
                font::draw_label_pad(buf, width, height, user, Rcti { xmin: r.xmin + u, ..r }, s.brush.label(), theme::WCOL_MENU_TEXT, 0);
                icons::draw_icon(buf, width, height, user, Rcti { xmin: r.xmax + 1 - u * 3 / 4, ..r }, Icon::DownArrow, TEXT);
            }
            Action::Radius | Action::Strength => {
                let inner = if hot { lighten(SLIDER_INNER, 10) } else { SLIDER_INNER };
                draw_round_box(buf, width, height, user, r, inner, outline, round);
                let frac = slider_fraction(item.action, s);
                let fill_w = ((r.size_x() - 1) as f32 * frac).round() as i32;
                if fill_w > 0 {
                    let fill = Rcti { xmin: r.xmin + 1, xmax: r.xmin + fill_w, ymin: r.ymin + 1, ymax: r.ymax - 1 };
                    crate::area::fill_rect(buf, width, height, fill, SLIDER_FILL.to_xrgb());
                }
                draw_round_box(buf, width, height, user, r, Rgba::hex(0x00000000), outline, round);
                let (name, value) = match item.action {
                    Action::Radius => ("Radius", format!("{:.0} px", s.radius_px)),
                    _ => ("Strength", format!("{:.3}", s.strength)),
                };
                font::draw_label(buf, width, height, user, r, name, TEXT);
                let vw = font::string_width(user, &value);
                font::draw_label_pad(buf, width, height, user, r, &value, TEXT, r.size_x() + 1 - vw - (6.0 * user.scale_factor) as i32);
            }
            Action::Add | Action::Subtract | Action::Symmetry(_) => {
                let on = match item.action {
                    Action::Add => !s.subtract,
                    Action::Subtract => s.subtract,
                    Action::Symmetry(a) => s.symmetry[a],
                    _ => false,
                };
                let inner = if on { theme::WCOL_TOGGLE_INNER_SEL } else { tool };
                draw_round_box_sides(buf, width, height, user, r, inner, outline, round, !item.left, !item.right);
                let text = match item.action {
                    Action::Add => "Add",
                    Action::Subtract => "Subtract",
                    Action::Symmetry(0) => "X",
                    Action::Symmetry(1) => "Y",
                    _ => "Z",
                };
                let tw = font::string_width(user, text);
                let col = if on { Rgba::hex(0xffffffff) } else { TEXT };
                font::draw_label_pad(buf, width, height, user, r, text, col, (r.size_x() + 1 - tw) / 2);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Rcti {
        Rcti { xmin: 2, xmax: 1195, ymin: 674, ymax: 699 }
    }

    #[test]
    fn items_fit_and_do_not_overlap() {
        let user = UserDef::from_window_scale(1.0);
        let h = header();
        let items = layout(&user, h);
        assert_eq!(items.len(), 8);
        for (i, a) in items.iter().enumerate() {
            assert!(a.rect.xmin >= h.xmin && a.rect.xmax <= h.xmax && a.rect.ymin >= h.ymin && a.rect.ymax <= h.ymax);
            for b in &items[i + 1..] {
                assert!(a.rect.isect(&b.rect).is_none(), "{:?} overlaps {:?}", a.action, b.action);
            }
        }
        assert!(rect_of(&items, Action::Brush).unwrap().xmin < rect_of(&items, Action::Radius).unwrap().xmin);
    }

    #[test]
    fn a_narrow_header_drops_what_does_not_fit() {
        let user = UserDef::from_window_scale(1.0);
        let items = layout(&user, Rcti { xmax: 400, ..header() });
        assert!(items.len() < 8 && rect_of(&items, Action::Brush).is_some());
        assert!(items.iter().all(|i| i.rect.xmax <= 400));
    }

    #[test]
    fn sliders_map_position_to_value() {
        let user = UserDef::from_window_scale(1.0);
        let items = layout(&user, header());
        let r = rect_of(&items, Action::Radius).unwrap();
        let mut s = Settings::default();
        assert!(set_slider(Action::Radius, r, r.xmin, &mut s));
        assert_eq!(s.radius_px, MIN_RADIUS_PX);
        set_slider(Action::Radius, r, r.xmax + 50, &mut s);
        assert_eq!(s.radius_px, MAX_RADIUS_PX, "dragging past the end clamps");
        set_slider(Action::Radius, r, r.xmin + r.size_x() / 2, &mut s);
        assert!((s.radius_px - 252.0).abs() < 8.0, "{}", s.radius_px);
        let st = rect_of(&items, Action::Strength).unwrap();
        set_slider(Action::Strength, st, st.xmin + st.size_x() / 4, &mut s);
        assert!((s.strength - 0.25).abs() < 0.02);
        assert!((slider_fraction(Action::Strength, &s) - s.strength).abs() < 1e-6);
        assert!(!set_slider(Action::Add, r, r.xmin, &mut s));
    }

    #[test]
    fn buttons_change_the_settings() {
        let mut s = Settings::default();
        assert!(press(Action::Subtract, false, &mut s) && s.subtract);
        assert!(!press(Action::Subtract, false, &mut s), "already subtracting");
        assert!(press(Action::Add, false, &mut s) && !s.subtract);
        assert!(press(Action::Symmetry(0), false, &mut s) && !s.symmetry[0]);
        assert!(press(Action::Symmetry(2), false, &mut s) && s.symmetry[2]);
        let first = s.brush;
        assert!(press(Action::Brush, false, &mut s));
        assert_ne!(s.brush, first);
        assert!(press(Action::Brush, true, &mut s));
        assert_eq!(s.brush, first, "Shift goes back");
    }

    #[test]
    fn cycling_visits_every_brush_once() {
        let mut b = Brush::Draw;
        let mut seen = Vec::new();
        for _ in 0..Brush::ALL.len() {
            seen.push(b);
            b = cycle_brush(b, false);
        }
        assert_eq!(b, Brush::Draw);
        seen.sort_by_key(|b| b.label());
        seen.dedup();
        assert_eq!(seen.len(), Brush::ALL.len());
    }

    #[test]
    fn drawing_changes_pixels_and_the_fill_follows_the_value() {
        if font_missing() {
            return;
        }
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (1200usize, 720usize);
        let bg = 0x0030_3030u32;
        let items = layout(&user, header());
        let mut lo = vec![bg; w * h];
        let mut hi = vec![bg; w * h];
        let mut s = Settings::default();
        s.strength = 0.1;
        draw(&mut lo, w, h, &user, &items, &s, None);
        s.strength = 0.9;
        draw(&mut hi, w, h, &user, &items, &s, None);
        assert!(lo.iter().any(|&p| p != bg));
        let blue = |b: &[u32]| b.iter().filter(|&&p| p == SLIDER_FILL.to_xrgb()).count();
        assert!(blue(&hi) > blue(&lo));
    }

    fn font_missing() -> bool {
        font::string_width(&UserDef::from_window_scale(1.0), "A") == 0
    }

    /* `cargo test dump_tool_header -- --ignored` writes /tmp/tool_header.ppm for a look at the header. */
    #[test]
    #[ignore]
    fn dump_tool_header() {
        let user = UserDef::from_window_scale(2.0);
        let (w, h) = (1500usize, 90usize);
        let bg = 0x0028_2828u32;
        let mut buf = vec![bg; w * h];
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 20, ymax: 20 + 25 * 2 };
        crate::area::blend_rect(&mut buf, w, h, region, theme::SPACE_VIEW3D_HEADER);
        let items = layout(&user, region);
        let mut s = Settings::default();
        s.radius_px = 120.0;
        s.strength = 0.65;
        draw(&mut buf, w, h, &user, &items, &s, Some(Action::Radius));
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        for y in 0..h {
            for x in 0..w {
                let p = buf[y * w + x];
                out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
            }
        }
        std::fs::write("/tmp/tool_header.ppm", out).unwrap();
    }
}
