//! Software drawing of a rounded widget box (inner fill and outline).
//!
//! Follows the SDF of `widget_base` as already used by `screen_draw::draw_area_outline()`:
//! the corner radius is `wcol->roundness * U.widget_unit` (interface_widgets.cc:3384, 6199) and
//! the outline is one pixel wide (`U.pixelsize`). Colours are blended in sRGB 8 bit with
//! `GPU_BLEND_ALPHA`, like the area outline. Emboss, gradients and text are not drawn.

use crate::area::Rcti;
use crate::theme::Rgba;
use crate::userdef::UserDef;

pub(crate) fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Alpha blends `color` with `alpha` over a `0x00RRGGBB` pixel.
pub(crate) fn blend_px(dst: u32, color: Rgba, alpha: f32) -> u32 {
    let ch = |shift: u32, c: u8| {
        let d = ((dst >> shift) & 0xff) as f32;
        ((c as f32 * alpha + d * (1.0 - alpha) + 0.5) as u32).min(255) << shift
    };
    ch(16, color.r) | ch(8, color.g) | ch(0, color.b)
}

/// Draws a rounded box covering the inclusive window `rect` (y up) into the buffer.
#[allow(clippy::too_many_arguments)]
pub fn draw_round_box(
    buf: &mut [u32],
    width: usize,
    height: usize,
    user: &UserDef,
    rect: Rcti,
    inner: Rgba,
    outline: Rgba,
    roundness: f32,
) {
    draw_round_box_sides(buf, width, height, user, rect, inner, outline, roundness, true, true);
}

/// Like `draw_round_box()` with the left and / or right corners squared off, for the buttons of an
/// aligned row (`UI_BUT_ALIGN_LEFT` / `UI_BUT_ALIGN_RIGHT` clear the corner flags, `UI_CNR_*`).
#[allow(clippy::too_many_arguments)]
pub fn draw_round_box_sides(
    buf: &mut [u32],
    width: usize,
    height: usize,
    user: &UserDef,
    rect: Rcti,
    inner: Rgba,
    outline: Rgba,
    roundness: f32,
    round_left: bool,
    round_right: bool,
) {
    draw_round_box_corners(buf, width, height, user, rect, inner, outline, roundness, [round_left, round_right, round_left, round_right]);
}

/// Like `draw_round_box()` with each corner switchable, `[bottom left, bottom right, top left, top right]` (y up),
/// as `UI_draw_roundbox_corner_set` does for the panel header and body (`UI_CNR_*`).
#[allow(clippy::too_many_arguments)]
pub fn draw_round_box_corners(
    buf: &mut [u32],
    width: usize,
    height: usize,
    user: &UserDef,
    rect: Rcti,
    inner: Rgba,
    outline: Rgba,
    roundness: f32,
    corners: [bool; 4],
) {
    if width == 0 || height == 0 || buf.len() < width * height || rect.is_empty() {
        return;
    }
    let size_x = (rect.size_x() + 1) as f32;
    let size_y = (rect.size_y() + 1) as f32;
    let rad_full = (roundness * user.widget_unit as f32).min(size_x.min(size_y) * 0.5);
    let line_w = (user.pixelsize as f32).max(1.0);
    let inner_a = inner.a as f32 / 255.0;
    let outline_a = outline.a as f32 / 255.0;

    let Some(vis) = rect.isect(&Rcti { xmin: 0, xmax: width as i32 - 1, ymin: 0, ymax: height as i32 - 1 }) else {
        return;
    };
    for y in vis.ymin..=vis.ymax {
        let v = (y - rect.ymin) as f32 + 0.5;
        let row = (height - 1 - y as usize) * width;
        for x in vis.xmin..=vis.xmax {
            let u = (x - rect.xmin) as f32 + 0.5;
            let rounded = corners[(if v > size_y * 0.5 { 2 } else { 0 }) + usize::from(u > size_x * 0.5)];
            let rad = if rounded { rad_full } else { 0.0 };
            let ux = if u > size_x * 0.5 { size_x - u } else { u } - rad;
            let uy = if v > size_y * 0.5 { size_y - v } else { v } - rad;
            let inner_sdf = ux.min(uy).max(0.0);
            let outer_sdf = -(ux.min(0.0).powi(2) + uy.min(0.0).powi(2)).sqrt();
            let sdf = inner_sdf + outer_sdf + rad;

            let mask_border = smoothstep(-0.5, 0.5, sdf);
            let mask_inner = smoothstep(-0.5, 0.5, sdf - line_w);
            let px = &mut buf[row + x as usize];
            let band = outline_a * (mask_border - mask_inner).max(0.0);
            if band > 0.0 {
                *px = blend_px(*px, outline, band);
            }
            let fill = inner_a * mask_inner;
            if fill > 0.0 {
                *px = blend_px(*px, inner, fill);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{WCOL_TOOL_INNER, WCOL_TOOL_OUTLINE, WCOL_TOOL_ROUNDNESS};

    /* A 40x20 box: the centre takes the inner colour, the outermost corner pixel keeps the
     * background (it is outside the rounded corner), and nothing is drawn outside the rect. */
    #[test]
    fn box_fills_centre_and_rounds_corner() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (60usize, 40usize);
        let bg = 0x0018_1818u32;
        let mut buf = vec![bg; w * h];
        let rect = Rcti { xmin: 10, xmax: 49, ymin: 10, ymax: 29 };
        draw_round_box(&mut buf, w, h, &user, rect, WCOL_TOOL_INNER, WCOL_TOOL_OUTLINE, WCOL_TOOL_ROUNDNESS);

        let at = |x: usize, y: usize| buf[(h - 1 - y) * w + x];
        assert_eq!(at(30, 20), WCOL_TOOL_INNER.to_xrgb());
        assert_eq!(at(10, 10), bg);
        assert_eq!(at(5, 20), bg);
        assert_ne!(at(30, 10), bg);
    }

    #[test]
    fn single_corners_can_stay_square() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (60usize, 40usize);
        let bg = 0x0018_1818u32;
        let mut buf = vec![bg; w * h];
        let rect = Rcti { xmin: 10, xmax: 49, ymin: 10, ymax: 29 };
        /* Only the top corners round: the bottom corner pixel is covered, the top one is not. */
        draw_round_box_corners(&mut buf, w, h, &user, rect, WCOL_TOOL_INNER, WCOL_TOOL_OUTLINE, WCOL_TOOL_ROUNDNESS, [false, false, true, true]);
        let at = |x: usize, y: usize| buf[(h - 1 - y) * w + x];
        assert_ne!(at(10, 10), bg);
        assert_eq!(at(10, 29), bg);
    }
}
