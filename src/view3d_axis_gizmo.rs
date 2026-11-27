//! Navigation axis gizmo of the 3D viewport, the layout part of `gizmo_axis_draw()`
//! (editors/space_view3d/view3d_gizmo_navigate_type.cc:55-290): the order, positions, depth and colours of
//! the six axis balls. `m` is the 3x3 view rotation (`gz->matrix_offset`, `m[col][row]`).

use crate::theme::{Rgba, TUI_XAXIS, TUI_YAXIS, TUI_ZAXIS};

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
}
