//! Navigation axis gizmo of the 3D viewport, the layout part of `gizmo_axis_draw()`
//! (editors/space_view3d/view3d_gizmo_navigate_type.cc:55-290): the order, positions, depth and colours of
//! the six axis balls. `m` is the 3x3 view rotation (`gz->matrix_offset`, `m[col][row]`).

use crate::area::Rcti;
use crate::font::{draw_text, string_width};
use crate::theme::{Rgba, SPACE_VIEW3D_BACK, TUI_XAXIS, TUI_YAXIS, TUI_ZAXIS};
use crate::userdef::UserDef;
use crate::widget_draw::blend_px;

/// `AXIS_HANDLE_SIZE`: the ball radius relative to the widget radius.
pub const AXIS_HANDLE_SIZE: f32 = 0.20;
/// `AXIS_DEPTH_BIAS`.
pub const AXIS_DEPTH_BIAS: f32 = 0.01;

/// One entry of `axis_order[6]`, in draw order (back to front).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisBall {
    /// `index`: 0/1 = -X/+X, 2/3 = -Y/+Y, 4/5 = -Z/+Z.
    pub index: usize,
    pub axis: usize,
    pub axis_opposite: usize,
    pub is_pos: bool,
    /// Rotated depth of the axis end (`-m[axis][2]` for the negative end).
    pub depth: f32,
}

impl AxisBall {
    /// `is_behind`.
    pub fn is_behind(&self) -> bool {
        self.depth <= AXIS_DEPTH_BIAS * if self.is_pos { -1.0 } else { 1.0 }
    }

    /// `v_final`: the ball centre before the rotation, `(1 - AXIS_HANDLE_SIZE)` along the signed axis.
    pub fn v_final(&self) -> [f32; 3] {
        let mut v = [0.0; 3];
        v[self.axis] = (1.0 - AXIS_HANDLE_SIZE) * if self.is_pos { 1.0 } else { -1.0 };
        v
    }

    /// The ball centre after the rotation `m` (x, y screen offset, z depth), in units of the widget radius.
    pub fn position(&self, m: [[f32; 3]; 3]) -> [f32; 3] {
        let v = self.v_final();
        let mut out = [0.0; 3];
        for (row, o) in out.iter_mut().enumerate() {
            *o = m[0][row] * v[0] + m[1][row] * v[1] + m[2][row] * v[2];
        }
        out
    }

    /// Ball size factor from back to front, 0.92 - 1.08.
    pub fn scale(&self) -> f32 {
        (self.depth + 1.0) * 0.08 + 0.92
    }
}

/// `axis_align`: the axis pointing at the viewer (an axis view), if any.
pub fn axis_align(m: [[f32; 3]; 3]) -> Option<usize> {
    (0..3).find(|&a| m[a][0] * m[a][0] + m[a][1] * m[a][1] < 1e-6)
}

/// `axis_order[]` after the `qsort` by depth (back to front; stable on ties).
pub fn axis_order(m: [[f32; 3]; 3]) -> [AxisBall; 6] {
    let mut order = [AxisBall { index: 0, axis: 0, axis_opposite: 0, is_pos: false, depth: 0.0 }; 6];
    for axis in 0..3 {
        order[axis * 2] = AxisBall { index: axis * 2, axis, axis_opposite: axis * 2 + 1, is_pos: false, depth: -m[axis][2] };
        order[axis * 2 + 1] = AxisBall { index: axis * 2 + 1, axis, axis_opposite: axis * 2, is_pos: true, depth: m[axis][2] };
    }
    order.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    order
}

/// `ui::theme::get_color_3fv(TH_AXIS_X + axis)` with alpha 1.
pub fn axis_color(axis: usize) -> [f32; 4] {
    let c: Rgba = [TUI_XAXIS, TUI_YAXIS, TUI_ZAXIS][axis];
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, 1.0]
}

/// `interp_v4_v4v4(a, b, t)`: `a * (1 - t) + b * t`.
pub fn interp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let s = 1.0 - t;
    [a[0] * s + b[0] * t, a[1] * s + b[1] * t, a[2] * s + b[2] * t, a[3] * s + b[3] * t]
}

/// `fading_color`: full at the front, half the view background at the back.
pub fn fading_color(view: [f32; 4], axis: usize, depth: f32) -> [f32; 4] {
    interp(view, axis_color(axis), (depth + 1.0) * 0.25 + 0.5)
}

/// `middle_color`: midway between front and back.
pub fn middle_color(view: [f32; 4], axis: usize) -> [f32; 4] {
    interp(view, axis_color(axis), 0.75)
}

/// `U.gizmo_size_navigate_v3d` and `GIZMO_OFFSET` (view3d_gizmo_navigate.cc:38-41).
const GIZMO_SIZE: f32 = 80.0;
const GIZMO_OFFSET: f32 = 10.0;

fn to_rgba(c: [f32; 4]) -> (Rgba, f32) {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    (Rgba { r: b(c[0]), g: b(c[1]), b: b(c[2]), a: 255 }, c[3].clamp(0.0, 1.0))
}

/// Blends `c` (alpha in `c[3]`) with `coverage` into the pixel at window `(x, y)`, y up.
fn put(buf: &mut [u32], w: usize, h: usize, x: i32, y: i32, c: [f32; 4], coverage: f32) {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 || coverage <= 0.0 {
        return;
    }
    let (rgb, a) = to_rgba(c);
    let i = (h - 1 - y as usize) * w + x as usize;
    buf[i] = blend_px(buf[i], rgb, coverage * a);
}

/// `draw_roundbox_4fv_ex` of a circle: `inner` disc with an `outline` ring of `ring` pixels.
fn ball(buf: &mut [u32], w: usize, h: usize, (cx, cy): (f32, f32), rad: f32, ring: f32, inner: [f32; 4], outline: [f32; 4]) {
    for y in (cy - rad - 1.0).floor() as i32..=(cy + rad + 1.0).ceil() as i32 {
        for x in (cx - rad - 1.0).floor() as i32..=(cx + rad + 1.0).ceil() as i32 {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let cov = (rad - d + 0.5).clamp(0.0, 1.0);
            let ring_t = (d - (rad - ring) + 0.5).clamp(0.0, 1.0);
            put(buf, w, h, x, y, interp(inner, outline, ring_t), cov);
        }
    }
}

/// `GPU_SHADER_3D_POLYLINE_SMOOTH_COLOR` line from `a` to `b` of `width` pixels, colour `ca` to `cb`.
fn line(buf: &mut [u32], w: usize, h: usize, a: (f32, f32), b: (f32, f32), width: f32, ca: [f32; 4], cb: [f32; 4]) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = (dx * dx + dy * dy).max(1e-6);
    let half = width / 2.0;
    for y in (a.1.min(b.1) - half - 1.0).floor() as i32..=(a.1.max(b.1) + half + 1.0).ceil() as i32 {
        for x in (a.0.min(b.0) - half - 1.0).floor() as i32..=(a.0.max(b.0) + half + 1.0).ceil() as i32 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let t = (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0);
            let d = ((px - (a.0 + dx * t)).powi(2) + (py - (a.1 + dy * t)).powi(2)).sqrt();
            put(buf, w, h, x, y, interp(ca, cb, t), (half - d + 0.5).clamp(0.0, 1.0));
        }
    }
}

/// `(centre x, centre y, radius)` in window pixels, y up (view3d_gizmo_navigate.cc:307-312, `WIDGET_RADIUS`).
pub fn placement(user: &UserDef, region: Rcti) -> (f32, f32, f32) {
    let s = user.scale_factor;
    let off = s * (GIZMO_OFFSET + GIZMO_SIZE / 2.0);
    (region.xmax as f32 - off, region.ymax as f32 - off, GIZMO_SIZE / 2.0 * s)
}

fn rot(m: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    let mut out = [0.0; 3];
    for (row, o) in out.iter_mut().enumerate() {
        *o = m[0][row] * v[0] + m[1][row] * v[1] + m[2][row] * v[2];
    }
    out
}

/// `gizmo_axis_draw()` without the hover state: lines, balls and letters, back to front.
/// Deviation: the letters use the UI font, not the bold default font.
pub fn draw(buf: &mut [u32], w: usize, h: usize, user: &UserDef, region: Rcti, m: [[f32; 3]; 3]) {
    let (cx, cy, rad) = placement(user, region);
    let view = {
        let c = SPACE_VIEW3D_BACK;
        [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, 1.0]
    };
    let line_width = GIZMO_SIZE / 40.0 * user.pixelsize as f32;
    let ring_width = GIZMO_SIZE / 60.0 * user.pixelsize as f32;
    let align = axis_align(m);
    let screen = |v: [f32; 3]| (cx + v[0] * rad, cy + v[1] * rad);
    for b in axis_order(m) {
        let behind = b.is_behind();
        let aligned_front = align == Some(b.axis) && !behind;
        let aligned_back = align == Some(b.axis) && behind;
        let at = screen(b.position(m));
        let col = axis_color(b.axis);
        let fading = fading_color(view, b.axis, b.depth);
        if b.is_pos || align.is_some() {
            let v = b.v_final();
            let k = -(line_width / rad * 0.66);
            let e = 1.0 - AXIS_HANDLE_SIZE;
            let start = screen(rot(m, [v[0] * k, v[1] * k, v[2] * k]));
            let end = screen(rot(m, [v[0] * e, v[1] * e, v[2] * e]));
            line(buf, w, h, start, end, line_width, middle_color(view, b.axis), fading);
        }
        if !aligned_back {
            let (mut inner, mut outline) = (fading, fading);
            if !b.is_pos {
                let alpha = (b.depth + 1.0).min(1.0);
                if aligned_front {
                    outline = interp([1.0; 4], col, 0.5);
                    outline[3] = alpha;
                } else {
                    inner = interp(view, col, 0.25);
                    inner[3] = alpha;
                }
            }
            ball(buf, w, h, at, rad * AXIS_HANDLE_SIZE * b.scale(), ring_width, inner, outline);
        }
        if (b.is_pos || align == Some(b.axis)) && !aligned_back {
            let name = format!("{}{}", if b.is_pos { "" } else { "-" }, ['X', 'Y', 'Z'][b.axis]);
            let tw = string_width(user, &name) as f32;
            let x = (at.0 - tw * if b.is_pos { 0.5 } else { 0.55 }).round() as i32;
            let baseline = (at.1 - rad * AXIS_HANDLE_SIZE * 1.25 * 0.36).round() as i32;
            draw_text(buf, w, h, user, x, baseline, &name, Rgba { r: 0, g: 0, b: 0, a: 230 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view3d::{quat_to_mat4, STARTUP_VIEWQUAT};

    fn m3(q: [f32; 4]) -> [[f32; 3]; 3] {
        let m = quat_to_mat4(q);
        [[m[0][0], m[0][1], m[0][2]], [m[1][0], m[1][1], m[1][2]], [m[2][0], m[2][1], m[2][2]]]
    }
    const IDENT: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    #[test]
    fn order_is_back_to_front_by_depth() {
        let order = axis_order(m3(STARTUP_VIEWQUAT));
        assert!(order.windows(2).all(|w| w[0].depth <= w[1].depth));
        let mut idx: Vec<usize> = order.iter().map(|b| b.index).collect();
        idx.sort();
        assert_eq!(idx, [0, 1, 2, 3, 4, 5]);
        /* Each axis' two ends have opposite depth. */
        for b in order {
            let o = order.iter().find(|x| x.index == b.axis_opposite).unwrap();
            assert!((b.depth + o.depth).abs() < 1e-6);
        }
    }

    #[test]
    fn identity_looks_down_z_and_aligns_to_z() {
        assert_eq!(axis_align(IDENT), Some(2));
        let order = axis_order(IDENT);
        assert_eq!((order[5].index, order[5].depth), (5, 1.0));
        assert_eq!((order[0].index, order[0].depth), (4, -1.0));
        assert!(order[0].is_behind() && !order[5].is_behind());
    }

    #[test]
    fn startup_view_is_not_axis_aligned_and_z_ball_is_up() {
        let m = m3(STARTUP_VIEWQUAT);
        assert_eq!(axis_align(m), None);
        let z = axis_order(m).into_iter().find(|b| b.index == 5).unwrap();
        let p = z.position(m);
        assert!(p[1] > 0.5, "+Z ball is above the centre");
        assert!((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 0.8 < 1e-5);
    }

    #[test]
    fn ball_positions_follow_v_final() {
        let b = axis_order(IDENT).into_iter().find(|b| b.index == 1).unwrap();
        assert_eq!(b.v_final(), [0.8, 0.0, 0.0]);
        assert_eq!(b.position(IDENT), [0.8, 0.0, 0.0]);
        let n = axis_order(IDENT).into_iter().find(|b| b.index == 2).unwrap();
        assert_eq!(n.v_final(), [0.0, -0.8, 0.0]);
    }

    #[test]
    fn colours_are_the_theme_axis_colours_faded_by_depth() {
        assert_eq!(axis_color(0), [1.0, 51.0 / 255.0, 82.0 / 255.0, 1.0]);
        let view = [0.0, 0.0, 0.0, 1.0];
        assert_eq!(fading_color(view, 2, 1.0), axis_color(2));
        let back = fading_color(view, 2, -1.0);
        assert!((back[2] - axis_color(2)[2] * 0.5).abs() < 1e-6);
        assert!((middle_color(view, 1)[1] - axis_color(1)[1] * 0.75).abs() < 1e-6);
    }

    #[test]
    fn draw_paints_the_z_ball_and_stays_inside_the_gizmo() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (400usize, 400usize);
        let region = Rcti { xmin: 0, xmax: 399, ymin: 0, ymax: 399 };
        let bg = 0x003d_3d3du32;
        let mut buf = vec![bg; w * h];
        let m = m3(STARTUP_VIEWQUAT);
        draw(&mut buf, w, h, &user, region, m);
        let (cx, cy, rad) = placement(&user, region);
        assert_eq!((cx, cy, rad), (349.0, 349.0, 40.0));
        let z = axis_order(m).into_iter().find(|b| b.index == 5).unwrap().position(m);
        let (x, y) = ((cx + z[0] * rad) as usize, (cy + z[1] * rad) as usize);
        assert_ne!(buf[(h - 1 - y) * w + x], bg);
        assert_eq!(buf[(h - 1 - 5) * w + 5], bg);
    }
}
