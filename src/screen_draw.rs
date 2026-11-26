//! Area border drawing (the rounded corners of editor areas).
//!
//! Ported from Blender:
//!  - `CORNER_RESOLUTION`, `do_vert_pair()`, `batch_screen_edges_get()`
//!    (editors/screen/screen_draw.cc:39-97)
//!  - `drawscredge_area()`              (editors/screen/screen_draw.cc:102)
//!  - `ED_screen_draw_edges()`          (editors/screen/screen_draw.cc:114-222, border part)
//!  - `EDITORRADIUS`                    (editors/screen/screen_intern.hh:84)
//!  - vertex + fragment stage of `area_borders`
//!    (gpu/shaders/gpu_shader_area_borders.bsl.hh)
//!
//! The GPU triangle strip is rasterised on the CPU. The rasteriser uses 8-bit
//! sub-pixel fixed point, pixel-centre sampling and the top-left fill rule, so
//! triangles that share an edge never blend a pixel twice. Barycentric
//! interpolation of `uv` replaces the shader varying (`w` is always 1, so it is
//! affine). Work is clipped to the scissor rectangle first, so triangles that
//! lie outside it cost nothing, and the per-pixel edge functions step
//! incrementally along each row.
//!
//! Also ported: the `editor_outline` round-box drawn after the borders
//! (screen_draw.cc:222-233 -> `draw_roundbox_4fv_ex`, interface_draw.cc:75, rendered with
//! the SDF fragment stage of `widget_base`, gpu_shader_widget_base.bsl.hh). Only the
//! outline is drawn there (no inner fill, the emboss colour is zero).
//!
//! Active area: the outline uses `editor_outline_active` for the active area
//! (`(&area == active_area) ? outline2 : outline1`); the caller decides which area is active.

use std::sync::OnceLock;

use crate::area::Rcti;
use crate::theme;
use crate::userdef::UserDef;

/// `#define CORNER_RESOLUTION 3`
const CORNER_RESOLUTION: usize = 3;
/// `CORNER_RESOLUTION * 2 * 4 + 2` vertices in the triangle strip.
const NUM_VERTS: usize = CORNER_RESOLUTION * 2 * 4 + 2;
/// `*corner_len = CORNER_RESOLUTION * 2` (the `cornerLen` uniform).
const CORNER_LEN: usize = CORNER_RESOLUTION * 2;

/// `EDITORRADIUS` is `6.0f * UI_SCALE_FAC`.
const EDITOR_RADIUS: f32 = 6.0;

/// `corner_coverage[]` (screen_draw.cc:184), indexed by `U.border_width - 1`.
const CORNER_COVERAGE: [f32; 10] = [
    0.144, 0.25, 0.334, 0.40, 0.455, 0.5, 0.538, 0.571, 0.6, 0.625,
];

/// `do_vert_pair()`: appends an (inner, outer) vertex pair.
fn do_vert_pair(verts: &mut Vec<[f32; 2]>, corner: usize, i: usize) {
    let angle = (corner as f64 * std::f64::consts::FRAC_PI_2
        + (i as f64 * std::f64::consts::FRAC_PI_2 / (CORNER_RESOLUTION as f64 - 1.0)))
        as f32;
    let mut inter = [angle.cos(), angle.sin()];

    /* Snap point to edge. */
    let div = 1.0 / inter[0].abs().max(inter[1].abs());
    let exter = [(inter[0] * div).round(), (inter[1] * div).round()];

    if i == 0 || i == CORNER_RESOLUTION - 1 {
        inter = exter;
    }

    /* Small offset to be able to tell inner and outer vertex apart inside the shader. */
    inter = [inter[0] * (1.0 - 0.0001), inter[1] * (1.0 - 0.0001)];

    verts.push(inter);
    verts.push(exter);
}

/// `batch_screen_edges_get()`: the static triangle strip, built once.
fn screen_edges_batch() -> &'static [[f32; 2]] {
    static BATCH: OnceLock<Vec<[f32; 2]>> = OnceLock::new();
    BATCH.get_or_init(|| {
        let mut verts = Vec::with_capacity(NUM_VERTS);
        for corner in 0..4 {
            for c in 0..CORNER_RESOLUTION {
                do_vert_pair(&mut verts, corner, c);
            }
        }
        /* Close the loop. */
        do_vert_pair(&mut verts, 0, 0);
        debug_assert_eq!(verts.len(), NUM_VERTS);
        verts
    })
}

/// Vertex stage of `area_borders::main_vert`.
/// `rect` is (xmin, xmax, ymin, ymax). Returns (screen position, uv).
#[inline]
fn vertex_stage(
    vert_id: usize,
    pos: [f32; 2],
    rect: [f32; 4],
    scale: f32,
    width: f32,
) -> ([f32; 2], [f32; 2]) {
    let corner_id = (vert_id / CORNER_LEN) % 4;
    let inner = pos[0].abs() < 1.0 && pos[1].abs() < 1.0;

    /* Scale the inner part of the border. Add a sub pixel offset to the outer part. */
    let k = if inner { 1.0 - width } else { 1.05 };
    let uv = [pos[0] * k, pos[1] * k];

    /* Rescale to the corner size and position the corner. */
    let (off, anchor) = match corner_id {
        0 => ([1.0, 1.0], [rect[1], rect[3]]),   /* top right: rect.yw */
        1 => ([-1.0, 1.0], [rect[0], rect[3]]),  /* top left: rect.xw */
        2 => ([-1.0, -1.0], [rect[0], rect[2]]), /* bottom left: rect.xz */
        _ => ([1.0, -1.0], [rect[1], rect[2]]),  /* bottom right: rect.yz */
    };
    (
        [(uv[0] - off[0]) * scale + anchor[0], (uv[1] - off[1]) * scale + anchor[1]],
        uv,
    )
}

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn fix(v: f32) -> i64 {
    (v * 256.0).round() as i64
}

#[inline]
fn edge(a: [i64; 2], b: [i64; 2], p: [i64; 2]) -> i64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Top-left rule for a counter-clockwise triangle in y-up space, as a bias that
/// is added to the edge function (0 keeps `w == 0`, -1 rejects it).
#[inline]
fn fill_bias(a: [i64; 2], b: [i64; 2]) -> i64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    if dy < 0 || (dy == 0 && dx < 0) {
        0
    } else {
        -1
    }
}

struct Target<'a> {
    buf: &'a mut [u32],
    width: usize,
    height: usize,
    /// Inclusive scissor box (x0, x1, y0, y1), already clamped to the buffer.
    scissor: [i32; 4],
}

/// Rasterises one triangle with the fragment stage of `area_borders::main_frag`
/// and `GPU_BLEND_ALPHA` blending.
fn raster_triangle(
    t: &mut Target,
    mut p: [[i64; 2]; 3],
    mut uv: [[f32; 2]; 3],
    color: [f32; 3],
    color_alpha: f32,
    scale: f32,
    width: f32,
) {
    let mut area2 = edge(p[0], p[1], p[2]);
    if area2 == 0 {
        return;
    }
    if area2 < 0 {
        p.swap(1, 2);
        uv.swap(1, 2);
        area2 = -area2;
    }

    let [sx0, sx1, sy0, sy1] = t.scissor;
    let min_x = (p[0][0].min(p[1][0]).min(p[2][0]) >> 8) as i32;
    let max_x = (p[0][0].max(p[1][0]).max(p[2][0]) >> 8) as i32;
    let min_y = (p[0][1].min(p[1][1]).min(p[2][1]) >> 8) as i32;
    let max_y = (p[0][1].max(p[1][1]).max(p[2][1]) >> 8) as i32;
    let (x0, x1) = (min_x.max(sx0), max_x.min(sx1));
    let (y0, y1) = (min_y.max(sy0), max_y.min(sy1));
    if x0 > x1 || y0 > y1 {
        return;
    }

    let [a, b, c] = p;
    let bias0 = fill_bias(b, c);
    let bias1 = fill_bias(c, a);
    let bias2 = fill_bias(a, b);

    /* Per-pixel step of each edge function along +x (one pixel = 256 units). */
    let dx0 = -(c[1] - b[1]) * 256;
    let dx1 = -(a[1] - c[1]) * 256;
    let dx2 = -(b[1] - a[1]) * 256;

    let inv_area = 1.0 / area2 as f32;
    let edge_offset = 0.98 - width;
    let src = [color[0] * 255.0, color[1] * 255.0, color[2] * 255.0];

    for y in y0..=y1 {
        let start = [x0 as i64 * 256 + 128, y as i64 * 256 + 128];
        let mut w0 = edge(b, c, start);
        let mut w1 = edge(c, a, start);
        let mut w2 = edge(a, b, start);
        let row = (t.height - 1 - y as usize) * t.width;

        for x in x0..=x1 {
            if ((w0 + bias0) | (w1 + bias1) | (w2 + bias2)) >= 0 {
                let (l0, l1, l2) = (w0 as f32 * inv_area, w1 as f32 * inv_area, w2 as f32 * inv_area);
                let u = l0 * uv[0][0] + l1 * uv[1][0] + l2 * uv[2][0];
                let v = l0 * uv[0][1] + l1 * uv[1][1] + l2 * uv[2][1];

                /* Fragment stage: minimize the AA on the edges. */
                let dist = ((u * u + v * v).sqrt() - edge_offset) * scale;
                let alpha = color_alpha * smoothstep(-0.09, 1.09, dist);

                if alpha > 0.0 {
                    let px = &mut t.buf[row + x as usize];
                    let d = *px;
                    let dr = ((d >> 16) & 0xff) as f32;
                    let dg = ((d >> 8) & 0xff) as f32;
                    let db = (d & 0xff) as f32;
                    let inv = 1.0 - alpha;
                    let r = (src[0] * alpha + dr * inv + 0.5) as u32;
                    let g = (src[1] * alpha + dg * inv + 0.5) as u32;
                    let bl = (src[2] * alpha + db * inv + 0.5) as u32;
                    *px = (r.min(255) << 16) | (g.min(255) << 8) | bl.min(255);
                }
            }
            w0 += dx0;
            w1 += dx1;
            w2 += dx2;
        }
    }
}

/// `drawscredge_area()` for one area, clipped to `scissor`.
///
/// `area` is `area.totrct` and `scissor` is the min/max of the area vertices
/// (`ED_screen_draw_edges()` builds it from `v1` and `v3` of every area; both
/// rectangles are inclusive pixel bounds, the scissor size is `size + 1`).
pub fn draw_area_border(
    buf: &mut [u32],
    width: usize,
    height: usize,
    user: &UserDef,
    area: Rcti,
    scissor: Rcti,
) {
    if width == 0 || height == 0 || buf.len() < width * height {
        return;
    }

    let scissor = [
        scissor.xmin.max(0),
        scissor.xmax.min(width as i32 - 1),
        scissor.ymin.max(0),
        scissor.ymax.min(height as i32 - 1),
    ];
    if scissor[0] > scissor[1] || scissor[2] > scissor[3] {
        return;
    }

    let ui_scale_fac = user.scale_factor;
    let edge_thickness = user.border_width as f32 * ui_scale_fac;

    /* Entire width of the evaluated outline as far as the shader is concerned. */
    let shader_scale = edge_thickness + EDITOR_RADIUS * ui_scale_fac;
    let shader_width = CORNER_COVERAGE[(user.border_width - 1).clamp(0, 9) as usize];

    /* BLI_rctf_rcti_copy() + BLI_rctf_pad(&rect, edge_thickness, edge_thickness). */
    let rect = [
        area.xmin as f32 - edge_thickness,
        area.xmax as f32 + edge_thickness,
        area.ymin as f32 - edge_thickness,
        area.ymax as f32 + edge_thickness,
    ];

    /* TH_EDITOR_BORDER. */
    let border = theme::EDITOR_BORDER;
    let color = [border.r as f32 / 255.0, border.g as f32 / 255.0, border.b as f32 / 255.0];
    let color_alpha = border.a as f32 / 255.0;

    let strip = screen_edges_batch();
    let mut screen = [[0i64; 2]; NUM_VERTS];
    let mut uvs = [[0f32; 2]; NUM_VERTS];
    for (i, pos) in strip.iter().enumerate() {
        let (s, uv) = vertex_stage(i, *pos, rect, shader_scale, shader_width);
        screen[i] = [fix(s[0]), fix(s[1])];
        uvs[i] = uv;
    }

    let mut target = Target { buf, width, height, scissor };

    /* GPU_PRIM_TRI_STRIP: triangle k is (k, k+1, k+2); the winding is normalised
     * in `raster_triangle`. */
    for k in 0..NUM_VERTS - 2 {
        raster_triangle(
            &mut target,
            [screen[k], screen[k + 1], screen[k + 2]],
            [uvs[k], uvs[k + 1], uvs[k + 2]],
            color,
            color_alpha,
            shader_scale,
            shader_width,
        );
    }
}

/// `ED_screen_draw_edges()` outline part: `draw_roundbox_4fv_ex(bounds, nullptr, nullptr, 1.0,
/// outline, U.pixelsize, EDITORRADIUS)` with `bounds = area.totrct` padded by 0.5 and all
/// corners rounded. The widget quad is `rect` outset by (0.5, 0.5) with an emboss offset of
/// `line_width` below; the fragment stage is evaluated per pixel centre and blended with
/// `GPU_BLEND_ALPHA`. Rows in the flat part of the box only touch the two edge bands.
pub fn draw_area_outline(
    buf: &mut [u32],
    width: usize,
    height: usize,
    user: &UserDef,
    area: Rcti,
    scissor: Rcti,
    active: bool,
) {
    if width == 0 || height == 0 || buf.len() < width * height {
        return;
    }
    let sc = [
        scissor.xmin.max(0),
        scissor.xmax.min(width as i32 - 1),
        scissor.ymin.max(0),
        scissor.ymax.min(height as i32 - 1),
    ];
    if sc[0] > sc[1] || sc[2] > sc[3] {
        return;
    }

    /* Outset by 1/2 pixel, regardless of UI scale or pixel size. */
    let padding = 0.5f32;
    let xmin = area.xmin as f32 - padding;
    let xmax = area.xmax as f32 + padding;
    let ymin = area.ymin as f32 - padding;
    let ymax = area.ymax as f32 + padding;
    let size_x = xmax - xmin;
    let size_y = ymax - ymin;

    let line_width = user.pixelsize as f32;
    let rad = EDITOR_RADIUS * user.scale_factor;

    let outline = if active { theme::EDITOR_OUTLINE_ACTIVE } else { theme::EDITOR_OUTLINE };
    let color = [outline.r as f32, outline.g as f32, outline.b as f32];
    let color_alpha = outline.a as f32 / 255.0;

    /* Quad covered by the widget: rect +- (0.5, 0.5), extended down by the emboss offset. */
    let qx0 = ((xmin - 0.5) - 0.5).ceil() as i32;
    let qx1 = ((xmax + 0.5) - 0.5).ceil() as i32 - 1;
    let qy0 = ((ymin - line_width - 0.5) - 0.5).ceil() as i32;
    let qy1 = ((ymax + 0.5) - 0.5).ceil() as i32 - 1;
    let (x0, x1) = (qx0.max(sc[0]), qx1.min(sc[1]));
    let (y0, y1) = (qy0.max(sc[2]), qy1.min(sc[3]));
    if x0 > x1 || y0 > y1 {
        return;
    }

    /* The fragment shader clamps the line width to at least 1px (fwidth(uv.y) == 1). */
    let line_w = line_width.max(1.0);
    let span = line_w.ceil() as i32 + 2;

    for y in y0..=y1 {
        let v = (y as f32 + 0.5) - ymin;
        let flat = v.min(size_y - v) > rad + line_w + 1.0;
        let ranges = if flat && qx1 - qx0 > 2 * span + 1 {
            [(qx0, qx0 + span), (qx1 - span, qx1)]
        } else {
            [(qx0, qx1), (1, 0)]
        };
        let row = (height - 1 - y as usize) * width;

        for (a, b) in ranges {
            for x in a.max(x0)..=b.min(x1) {
                let u = (x as f32 + 0.5) - xmin;

                /* Fragment stage of widget_base: signed distance to the rounded box. */
                let upper_half = v > size_y * 0.5;
                let right_half = u > size_x * 0.5;
                let ux = if right_half { size_x - u } else { u } - rad;
                let uy = if upper_half { size_y - v } else { v } - rad;
                let inner_sdf = ux.min(uy).max(0.0);
                let outer_sdf = -(ux.min(0.0).powi(2) + uy.min(0.0).powi(2)).sqrt();
                let sdf = inner_sdf + outer_sdf + rad;

                let mask_border = smoothstep(-0.5, 0.5, sdf);
                let mask_inner = smoothstep(-0.5, 0.5, sdf - line_w);
                let alpha = color_alpha * (mask_border - mask_inner).max(0.0);

                if alpha > 0.0 {
                    let px = &mut buf[row + x as usize];
                    let d = *px;
                    let dr = ((d >> 16) & 0xff) as f32;
                    let dg = ((d >> 8) & 0xff) as f32;
                    let db = (d & 0xff) as f32;
                    let inv = 1.0 - alpha;
                    let r = (color[0] * alpha + dr * inv + 0.5) as u32;
                    let g = (color[1] * alpha + dg * inv + 0.5) as u32;
                    let bl = (color[2] * alpha + db * inv + 0.5) as u32;
                    *px = (r.min(255) << 16) | (g.min(255) << 8) | bl.min(255);
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_has_expected_vertex_count() {
        assert_eq!(screen_edges_batch().len(), NUM_VERTS);
    }

    /* A 100x100 area filled with one colour: the corner pixel must be pulled towards
     * the border colour (it lies outside the rounded corner), the centre must not change. */
    #[test]
    fn corner_is_rounded_and_centre_untouched() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (100usize, 100usize);
        let fill = 0x003d3d3d;
        let mut buf = vec![fill; w * h];
        let area = Rcti { xmin: 0, xmax: 99, ymin: 0, ymax: 99 };
        draw_area_border(&mut buf, w, h, &user, area, area);

        assert_ne!(buf[(h - 1) * w], fill, "bottom-left corner pixel");
        assert_ne!(buf[0], fill, "top-left corner pixel");
        assert_eq!(buf[50 * w + 50], fill, "centre pixel");
    }

    /* The outline is a faint white line along the area edge: the edge pixel is lightened,
     * the centre is untouched. */
    #[test]
    fn outline_lightens_edge_only() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (100usize, 100usize);
        let fill = 0x003d3d3d;
        let mut buf = vec![fill; w * h];
        let area = Rcti { xmin: 0, xmax: 99, ymin: 0, ymax: 99 };
        draw_area_outline(&mut buf, w, h, &user, area, area, false);

        let edge = buf[(h - 1 - 50) * w];
        assert!((edge >> 16) & 0xff > 0x3d, "left edge pixel is lightened");
        assert_eq!(buf[50 * w + 50], fill, "centre pixel");
    }
}
