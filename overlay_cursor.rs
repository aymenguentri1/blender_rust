//! The 3D cursor of the 3D viewport overlay (`Cursor` in overlay_cursor.hh): the ring and the
//! axis lines.
//!
//! Ported from Blender:
//!  - `Cursor::begin_sync()`                (draw/engines/overlay/overlay_cursor.hh:37-127): the cursor
//!    location is projected with `ED_view3d_project_int_global(..., V3D_PROJ_TEST_CLIP_NEAR)`, the
//!    ring is drawn in region pixel space scaled by `U.widget_unit` with an orthographic projection
//!    offset by -0.01, line width `U.pixelsize`, `lineSmooth`, alpha blending and no depth test
//!  - `cursor_circle` batch                 (draw/engines/overlay/overlay_shape.cc:1318-1335): a
//!    12 segment line strip of radius 0.5, vertex colours alternate white (even) / red (odd)
//!  - `GPU_SHADER_3D_POLYLINE_FLAT_COLOR`   (gpu/shaders/gpu_shader_polyline.bsl.hh): each segment is
//!    a quad of half width `(lineWidth + 1) / 2` with the colour of its last vertex; the fragment
//!    alpha is multiplied by `clamp((lineWidth + 1) / 2 - |distance to the line|, 0, 1)`
//!
//!  - `cursor_lines` batch                  (draw/engines/overlay/overlay_shape.cc:1336-1368): per axis,
//!    the outer 15% of the positive half in the theme axis colour, the inner part in `TH_VIEW_OVERLAY`
//!    (not set by the default theme, so black), and the same for the negative half with the axis
//!    colour blended 25% to white and darkened by 60. The lines are drawn in world space with the
//!    cursor location and rotation, scaled by `ED_view3d_pixel_size_no_ui_scale() * U.widget_unit`
//!    and projected with the perspective matrices (overlay_cursor.hh:97-117)
//!
//! The fragment stage converts the colours with `rec709_srgb_to_output_space()`; the blend is done
//! in linear space like the grid. The cursor is always visible (overlay flags and paint modes are
//! not modelled).

use crate::area::Rcti;
use crate::overlay_grid::srgb_to_linearrgb;
use crate::theme::{Rgba, TUI_XAXIS, TUI_YAXIS, TUI_ZAXIS};
use crate::colorspace_lut::{linear_to_srgb8, srgb8_to_linear};
use crate::userdef::UserDef;
use crate::view3d::{quat_to_mat4, RegionView3D};

const SEGMENTS: usize = 12;
const RADIUS: f32 = 0.5;
const COLOR_PRIMARY: [f32; 3] = [1.0, 0.0, 0.0];
const COLOR_SECONDARY: [f32; 3] = [1.0, 1.0, 1.0];
/// `SMOOTH_WIDTH` of the polyline shader.
const SMOOTH_WIDTH: f32 = 1.0;
/// `ofs` of the orthographic projection (`wmOrtho2_region_pixelspace` copy).
const PROJ_OFS: f32 = -0.01;

/// The `cursor_circle` vertices: unit-radius position (x, y) and colour.
fn cursor_circle() -> Vec<([f32; 2], [f32; 3])> {
    (0..=SEGMENTS)
        .map(|i| {
            let angle = std::f32::consts::TAU * (i as f32 / SEGMENTS as f32);
            let color = if i % 2 == 0 { COLOR_SECONDARY } else { COLOR_PRIMARY };
            ([RADIUS * angle.cos(), RADIUS * angle.sin()], color)
        })
        .collect()
}

/// Rasterises one polyline segment `p0` -> `p1` (viewport pixels, region origin bottom-left) as the
/// quad of the polyline shader and alpha blends it into the buffer.
#[allow(clippy::too_many_arguments)]
pub fn draw_segment(
    buf: &mut [u32],
    width: usize,
    height: usize,
    region: Rcti,
    size: (i32, i32),
    p0: [f32; 2],
    p1: [f32; 2],
    color: [f32; 3],
    line_width: f32,
) {
    let lin = [srgb_to_linearrgb(color[0]), srgb_to_linearrgb(color[1]), srgb_to_linearrgb(color[2])];
    draw_segment_gradient(buf, width, height, region, size, p0, p1, lin, lin, line_width);
}

/// Like `draw_segment()`, but the colour (linear RGB) is interpolated from `c0` at `p0` to `c1` at
/// `p1` along the segment (the smooth `final_color` varying of the wireframe shader; interpolated
/// in screen space here, not perspective corrected).
#[allow(clippy::too_many_arguments)]
pub fn draw_segment_gradient(
    buf: &mut [u32],
    width: usize,
    height: usize,
    region: Rcti,
    (rw, rh): (i32, i32),
    p0: [f32; 2],
    p1: [f32; 2],
    c0: [f32; 3],
    c1: [f32; 3],
    line_width: f32,
) {
    let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return;
    }
    let e = [dx / len, dy / len];
    let half = (line_width + SMOOTH_WIDTH) * 0.5;

    let x0 = ((p0[0].min(p1[0]) - half).floor() as i32).max(0);
    let x1 = ((p0[0].max(p1[0]) + half).ceil() as i32).min(rw - 1);
    let y0 = ((p0[1].min(p1[1]) - half).floor() as i32).max(0);
    let y1 = ((p0[1].max(p1[1]) + half).ceil() as i32).min(rh - 1);

    let src_at = |s: f32| -> [f32; 3] { std::array::from_fn(|k| c0[k] + (c1[k] - c0[k]) * s) };
    for py in y0..=y1 {
        for px in x0..=x1 {
            let rel = [px as f32 + 0.5 - p0[0], py as f32 + 0.5 - p0[1]];
            let t = rel[0] * e[0] + rel[1] * e[1];
            /* Signed distance to the line, the `smoothline` varying. */
            let d = rel[0] * -e[1] + rel[1] * e[0];
            if t < 0.0 || t > len || d.abs() >= half {
                continue;
            }
            let a = (half - d.abs()).clamp(0.0, 1.0);

            let (x, y) = (region.xmin + px, region.ymin + py);
            if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
                continue;
            }
            let idx = (height - 1 - y as usize) * width + x as usize;
            let dst_px = buf[idx];
            let src = src_at((t / len).clamp(0.0, 1.0));
            let mut out = 0u32;
            for (i, shift) in [16u32, 8, 0].into_iter().enumerate() {
                let dst = srgb8_to_linear((dst_px >> shift) & 0xff);
                let lin = src[i] * a + dst * (1.0 - a);
                out |= linear_to_srgb8(lin) << shift;
            }
            buf[idx] = out;
        }
    }
}

/// `outer_limit`, `color_limit` and `inner_limit` of the `cursor_lines` batch.
const LINES_OUTER: f32 = 1.0;
const LINES_COLOR_LIMIT: f32 = 0.85;
const LINES_INNER: f32 = 0.25;
/// `TH_VIEW_OVERLAY`: not set by the default theme (`userdef_default_theme.c`), so black.
const VIEW_OVERLAY: [f32; 3] = [0.0, 0.0, 0.0];

/// `get_color_3fv()`.
fn color_3fv(c: Rgba) -> [f32; 3] {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]
}

/// `get_color_blend_shade_3fv(axis, TH_WHITE, 0.25, -60)`.
fn negative_axis_color(c: Rgba) -> [f32; 3] {
    let ch = |v: u8| (((0.75 * v as f32 + 0.25 * 255.0).floor() as i32) - 60).clamp(0, 255) as f32 / 255.0;
    [ch(c.r), ch(c.g), ch(c.b)]
}

/// The `cursor_lines` segments in cursor space: start, end and colour.
fn cursor_lines() -> Vec<([f32; 3], [f32; 3], [f32; 3])> {
    let mut lines = Vec::with_capacity(12);
    for (i, theme_color) in [TUI_XAXIS, TUI_YAXIS, TUI_ZAXIS].into_iter().enumerate() {
        let at = |k: f32| {
            let mut p = [0.0f32; 3];
            p[i] = k;
            p
        };
        lines.push((at(LINES_OUTER), at(LINES_COLOR_LIMIT), color_3fv(theme_color)));
        lines.push((at(LINES_COLOR_LIMIT), at(LINES_INNER), VIEW_OVERLAY));
        lines.push((at(-LINES_OUTER), at(-LINES_COLOR_LIMIT), negative_axis_color(theme_color)));
        lines.push((at(-LINES_COLOR_LIMIT), at(-LINES_INNER), VIEW_OVERLAY));
    }
    lines
}

/// Projects the world space segment `a` -> `b` with `persmat`, clipped to the near plane
/// (`z >= -w`), to region pixels (origin bottom-left). `None` when the whole segment is clipped.
pub fn project_segment(rv3d: &RegionView3D, a: [f32; 3], b: [f32; 3], size: (i32, i32)) -> Option<([f32; 2], [f32; 2])> {
    let p = &rv3d.persmat;
    let clip = |c: [f32; 3]| -> [f32; 4] {
        std::array::from_fn(|i| p[0][i] * c[0] + p[1][i] * c[1] + p[2][i] * c[2] + p[3][i])
    };
    let (ca, cb) = (clip(a), clip(b));
    let (da, db) = (ca[2] + ca[3], cb[2] + cb[3]);
    if da < 0.0 && db < 0.0 {
        return None;
    }
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    if da < 0.0 {
        t0 = da / (da - db);
    } else if db < 0.0 {
        t1 = da / (da - db);
    }
    let lerp = |t: f32| -> [f32; 4] { std::array::from_fn(|i| ca[i] + (cb[i] - ca[i]) * t) };
    let (c0, c1) = (lerp(t0), lerp(t1));
    if c0[3] <= 0.0 || c1[3] <= 0.0 {
        return None;
    }
    let screen = |c: [f32; 4]| {
        [((c[0] / c[3]) * 0.5 + 0.5) * size.0 as f32, ((c[1] / c[3]) * 0.5 + 0.5) * size.1 as f32]
    };
    Some((screen(c0), screen(c1)))
}


/// Draws the 3D cursor ring for `location` into `region` (inclusive Blender rect, y up) of the
/// `0x00RRGGBB` buffer. `rv3d` must have been updated for the size of the region.
pub fn draw_cursor(
    buf: &mut [u32],
    width: usize,
    height: usize,
    region: Rcti,
    user: &UserDef,
    rv3d: &RegionView3D,
    location: [f32; 3],
    rotation: [f32; 4],
) {
    let size = (region.size_x() + 1, region.size_y() + 1);
    if size.0 <= 0 || size.1 <= 0 {
        return;
    }
    /* Clipped: not drawn. */
    let Some(pixel) = rv3d.project_int_clip_near(size.0, size.1, location) else { return };

    /* `cursor_mat` translation, then the orthographic projection offset. */
    let center = [pixel[0] as f32 + 0.5 - PROJ_OFS, pixel[1] as f32 + 0.5 - PROJ_OFS];
    let scale = user.widget_unit as f32;
    let line_width = user.pixelsize as f32;

    let verts: Vec<[f32; 2]> = cursor_circle()
        .iter()
        .map(|(p, _)| [center[0] + scale * p[0], center[1] + scale * p[1]])
        .collect();
    let colors: Vec<[f32; 3]> = cursor_circle().iter().map(|(_, c)| *c).collect();
    for i in 0..SEGMENTS {
        /* Flat colour: the provoking (last) vertex of the segment. */
        draw_segment(buf, width, height, region, size, verts[i], verts[i + 1], colors[i + 1], line_width);
    }

    /* `cursor_mat = from_loc_rot_scale(location, rotation, scale * widget_unit)`. */
    let scale = rv3d.pixel_size_no_ui_scale(location) * scale;
    let rot = quat_to_mat4(rotation);
    let to_world = |p: [f32; 3]| -> [f32; 3] {
        let l = [p[0] * scale, p[1] * scale, p[2] * scale];
        std::array::from_fn(|i| location[i] + rot[0][i] * l[0] + rot[1][i] * l[1] + rot[2][i] * l[2])
    };
    for (p0, p1, color) in cursor_lines() {
        if let Some((s0, s1)) = project_segment(rv3d, to_world(p0), to_world(p1), size) {
            draw_segment(buf, width, height, region, size, s0, s1, color, line_width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view3d::View3D;

    /* 200x100 region with 20 px widget units: the ring is centred on pixel (100, 50) with a
     * radius of 10 px. The first segment (angle 0 to 30 degrees) is red; pixel (110, 51) lies a
     * quarter pixel off its centre line, so it is 75% red. The ring centre keeps the background. */
    #[test]
    fn ring_is_drawn_around_the_origin() {
        let (w, h) = (200usize, 100usize);
        let bg = crate::theme::SPACE_VIEW3D_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w as i32, h as i32);
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };
        let user = UserDef::from_window_scale(1.0);
        draw_cursor(&mut buf, w, h, region, &user, &rv3d, [0.0; 3], [1.0, 0.0, 0.0, 0.0]);

        let at = |x: usize, y: usize| buf[(h - 1 - y) * w + x];
        assert_eq!(at(100, 50), bg);
        let p = at(110, 51);
        let (r, g) = ((p >> 16) & 0xff, (p >> 8) & 0xff);
        assert!(r > 150 && g < 61, "{:#08x}", p);
    }

    #[test]
    fn clipped_cursor_is_not_drawn() {
        let (w, h) = (200usize, 100usize);
        let bg = crate::theme::SPACE_VIEW3D_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w as i32, h as i32);
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };
        let user = UserDef::from_window_scale(1.0);
        draw_cursor(&mut buf, w, h, region, &user, &rv3d, [0.0, 0.0, 1000.0], [1.0, 0.0, 0.0, 0.0]);
        assert!(buf.iter().all(|&p| p == bg));
    }

    /* 200x100 region, identity view and rotation: one widget unit is 20 px (1.44 world units at
     * distance 10), so the +X axis runs right of the centre: the inner part (5 to 17 px) is black and
     * darkens the background, the outer part (17 to 20 px) is red. The line is 1 px wide on the pixel
     * edge between rows 49 and 50, so both rows get half coverage. */
    #[test]
    fn axis_lines_are_drawn() {
        let (w, h) = (200usize, 100usize);
        let bg = crate::theme::SPACE_VIEW3D_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w as i32, h as i32);
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };
        let user = UserDef::from_window_scale(1.0);
        draw_cursor(&mut buf, w, h, region, &user, &rv3d, [0.0; 3], [1.0, 0.0, 0.0, 0.0]);

        let at = |x: usize, y: usize| buf[(h - 1 - y) * w + x];
        for y in [49, 50] {
            assert!((at(107, y) >> 16) & 0xff < 0x3d, "inner part, row {y}: {:#08x}", at(107, y));
            let p = at(118, y);
            let (r, g) = ((p >> 16) & 0xff, (p >> 8) & 0xff);
            assert!(r > g + 20, "outer part, row {y}: {:#08x}", p);
        }
    }
}
