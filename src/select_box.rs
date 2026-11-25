//! Box select (`VIEW3D_OT_select_box`, B and a click-drag on empty space) and the dashed border drawn by
//! `WM_gesture_box_draw()`.
//!
//! Modes follow `SEL_OP_*`: a plain drag replaces the selection, Shift extends it and Ctrl subtracts.
//! Deviations: the rectangle is tested against the projected triangles of the evaluated meshes (X-ray and
//! wire shading) or against a grid of pick rays (solid shading, which honours occlusion like Blender's
//! depth-buffer id pass but can miss an object smaller than the grid step); cameras, lights and empties
//! count when their origin lies inside the rectangle.

use crate::area::Rcti;

/// `SEL_OP_ADD` / `SEL_OP_SUB` / `SEL_OP_SET`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxMode {
    Set,
    Add,
    Sub,
}

impl BoxMode {
    /// The mode of a press with the given modifier keys (Ctrl wins over Shift, as in the keymap).
    pub fn from_keys(shift: bool, ctrl: bool) -> BoxMode {
        if ctrl {
            BoxMode::Sub
        } else if shift {
            BoxMode::Add
        } else {
            BoxMode::Set
        }
    }
}

/// Drag distance in pixels before a press on empty space becomes a box (`WM_EVENT_CURSOR_DRAG_THRESHOLD`).
pub const DRAG_THRESHOLD: i32 = 3;

/// The border of two region points as `[xmin, xmax, ymin, ymax]`.
pub fn rect_of(a: [i32; 2], b: [i32; 2]) -> [i32; 4] {
    [a[0].min(b[0]), a[0].max(b[0]), a[1].min(b[1]), a[1].max(b[1])]
}

fn cross(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn segments_cross(p: [f32; 2], q: [f32; 2], r: [f32; 2], s: [f32; 2]) -> bool {
    let (d1, d2) = (cross(p, q, r), cross(p, q, s));
    let (d3, d4) = (cross(r, s, p), cross(r, s, q));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) && d1 != 0.0 && d2 != 0.0 && d3 != 0.0 && d4 != 0.0
}

fn point_in_triangle(t: &[[f32; 2]; 3], p: [f32; 2]) -> bool {
    let (a, b, c) = (cross(t[0], t[1], p), cross(t[1], t[2], p), cross(t[2], t[0], p));
    (a >= 0.0 && b >= 0.0 && c >= 0.0) || (a <= 0.0 && b <= 0.0 && c <= 0.0)
}

/// Does the screen space triangle touch the (inclusive pixel) rectangle `[xmin, xmax, ymin, ymax]`?
pub fn triangle_hits_rect(t: &[[f32; 2]; 3], rect: [i32; 4]) -> bool {
    let (x0, x1, y0, y1) = (rect[0] as f32, rect[1] as f32 + 1.0, rect[2] as f32, rect[3] as f32 + 1.0);
    if t.iter().any(|p| p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1) {
        return true;
    }
    let corners = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    if corners.iter().any(|&c| point_in_triangle(t, c)) {
        return true;
    }
    for i in 0..3 {
        for k in 0..4 {
            if segments_cross(t[i], t[(i + 1) % 3], corners[k], corners[(k + 1) % 4]) {
                return true;
            }
        }
    }
    false
}

/// The sample step (pixels) of the pick ray grid so that a rectangle costs at most about 2500 rays.
pub fn sample_step(rect: [i32; 4]) -> i32 {
    let (w, h) = ((rect[1] - rect[0] + 1).max(1) as f32, (rect[3] - rect[2] + 1).max(1) as f32);
    ((w * h / 2500.0).sqrt().ceil() as i32).max(1)
}

/// `WM_gesture_box_draw()`: the dashed rectangle (black and white 4 px dashes) in region pixels, y up.
pub fn draw_box(buf: &mut [u32], width: usize, height: usize, region: Rcti, a: [i32; 2], b: [i32; 2]) {
    let r = rect_of(a, b);
    let mut put = |x: i32, y: i32, i: i32| {
        let (px, py) = (x + region.xmin, y + region.ymin);
        if px < region.xmin || px > region.xmax || py < region.ymin || py > region.ymax {
            return;
        }
        if px < 0 || py < 0 || px as usize >= width || py as usize >= height {
            return;
        }
        let row = height - 1 - py as usize;
        buf[row * width + px as usize] = if (i / 4) % 2 == 0 { 0xffffff } else { 0x000000 };
    };
    for x in r[0]..=r[1] {
        put(x, r[2], x - r[0]);
        put(x, r[3], x - r[0]);
    }
    for y in r[2]..=r[3] {
        put(r[0], y, y - r[2]);
        put(r[1], y, y - r[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_from_keys() {
        assert_eq!(BoxMode::from_keys(false, false), BoxMode::Set);
        assert_eq!(BoxMode::from_keys(true, false), BoxMode::Add);
        assert_eq!(BoxMode::from_keys(true, true), BoxMode::Sub);
        assert_eq!(BoxMode::from_keys(false, true), BoxMode::Sub);
    }

    #[test]
    fn triangle_inside_around_and_crossing() {
        let rect = [10, 20, 10, 20];
        assert!(triangle_hits_rect(&[[12.0, 12.0], [15.0, 12.0], [12.0, 15.0]], rect));
        assert!(triangle_hits_rect(&[[0.0, 0.0], [100.0, 0.0], [0.0, 100.0]], rect));
        assert!(triangle_hits_rect(&[[0.0, 15.0], [30.0, 14.0], [30.0, 16.0]], rect));
        assert!(!triangle_hits_rect(&[[30.0, 30.0], [40.0, 30.0], [30.0, 40.0]], rect));
        assert!(!triangle_hits_rect(&[[0.0, 0.0], [5.0, 0.0], [0.0, 5.0]], rect));
    }

    #[test]
    fn step_grows_with_area() {
        assert_eq!(sample_step([0, 9, 0, 9]), 1);
        assert!(sample_step([0, 999, 0, 999]) >= 20);
    }
}
