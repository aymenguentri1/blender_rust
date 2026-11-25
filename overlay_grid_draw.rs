//! Software drawing of the overlay grid pass into the 3D viewport main region.
//!
//! Ported from Blender:
//!  - the draw sequence of `Grid::begin_sync()` / `Grid::draw_line()`
//!                                      (draw/engines/overlay/overlay_grid.hh:44-131): for each of
//!    `num_iters` iterations the axis pass (6 vertices) then the grid pass, as `GPU_PRIM_LINES`
//!    with `DRW_STATE_DEPTH_LESS_EQUAL`, depth written only in iteration 0
//!  - blend state: `DRW_STATE_BLEND_ALPHA | DRW_STATE_BLEND_ADD` ORs to the `BLEND_ALPHA` bits
//!    (draw/intern/draw_state.hh:53-57, `to_blend()` at :178), so the blend is `GPU_BLEND_ALPHA`:
//!    `src.rgb * src.a + dst.rgb * (1 - src.a)` (gpu/opengl/gl_state.cc:313)
//!  - provoking vertex is `GPU_VERTEX_LAST` (draw_state.hh:211), so the flat `edge_start` of the
//!    fragment stage is the screen position of a line's second vertex
//!  - `linearrgb_to_srgb()`             (blenlib/intern/math_color.cc:468)
//!
//! Approximations of the GPU pipeline: 1 pixel wide lines are rasterised by stepping the major
//! axis at pixel centres (no exact diamond-exit rule), the depth buffer is cleared to 1.0 and the
//! buffer is blended in linear space and stored as sRGB (the "Standard" view transform).

use crate::area::Rcti;
use crate::colorspace_lut::{linear_to_srgb8, srgb8_to_linear};
use crate::overlay_grid::{
    grid_colors, grid_fragment, grid_init_v3d, grid_pass_lines, screen_position,
    GridColors, GridFragIn, GridVertex,
};
use crate::view3d::{RegionView3D, View3D};

/// `linearrgb_to_srgb()`.
pub fn linearrgb_to_srgb(c: f32) -> f32 {
    if c < 0.003_130_8 {
        return if c < 0.0 { 0.0 } else { c * 12.92 };
    }
    1.055 * c.powf(1.0 / 2.4) - 0.055
}

fn lerp<const N: usize>(a: [f32; N], b: [f32; N], t: f32) -> [f32; N] {
    let mut r = a;
    for i in 0..N {
        r[i] = a[i] + (b[i] - a[i]) * t;
    }
    r
}

/// Clips a segment in clip space against `-w <= x, y, z <= w`; returns the kept parameter range.
fn clip_line(a: [f32; 4], b: [f32; 4]) -> Option<(f32, f32)> {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for axis in 0..3 {
        for sign in [1.0f32, -1.0] {
            let da = a[3] + sign * a[axis];
            let db = b[3] + sign * b[axis];
            if da < 0.0 && db < 0.0 {
                return None;
            }
            if da < 0.0 {
                t0 = t0.max(da / (da - db));
            } else if db < 0.0 {
                t1 = t1.min(da / (da - db));
            }
        }
    }
    if t0 > t1 {
        None
    } else {
        Some((t0, t1))
    }
}

/// The region being drawn: the window buffer, the region rectangle and its depth buffer.
struct Target<'a> {
    buf: &'a mut [u32],
    width: usize,
    height: usize,
    region: Rcti,
    rw: i32,
    rh: i32,
    depth: Vec<f32>,
}

impl Target<'_> {
    /// Depth test (`LESS_EQUAL`), fragment stage, optional depth write and alpha blend.
    #[allow(clippy::too_many_arguments)]
    fn fragment(
        &mut self,
        colors: &GridColors,
        grid_flag: u32,
        grid_iter: u32,
        px: i32,
        py: i32,
        z: f32,
        frag: &GridFragIn,
        rv3d: &RegionView3D,
    ) {
        if px < 0 || py < 0 || px >= self.rw || py >= self.rh {
            return;
        }
        let di = (py * self.rw + px) as usize;
        if z > self.depth[di] {
            return;
        }
        let Some(c) = grid_fragment(colors, grid_flag, grid_iter, frag, rv3d) else { return };
        /* Only the first iteration writes depth. */
        if grid_iter == 0 {
            self.depth[di] = z;
        }

        let x = self.region.xmin + px;
        let y = self.region.ymin + py;
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let idx = (self.height - 1 - y as usize) * self.width + x as usize;
        let a = c[3];
        if a <= 0.0 {
            return;
        }
        let d = self.buf[idx];
        let mut out = 0u32;
        for (i, shift) in [16u32, 8, 0].into_iter().enumerate() {
            let dst = srgb8_to_linear((d >> shift) & 0xff);
            let lin = c[i] * a + dst * (1.0 - a);
            out |= linear_to_srgb8(lin) << shift;
        }
        self.buf[idx] = out;
    }

    /// Clips, rasterises and shades one line of a pass.
    fn line(
        &mut self,
        colors: &GridColors,
        grid_flag: u32,
        grid_iter: u32,
        line: &[GridVertex; 2],
        rv3d: &RegionView3D,
    ) {
        let [va, vb] = line;
        let Some((t0, t1)) = clip_line(va.clip, vb.clip) else { return };
        let c0 = lerp(va.clip, vb.clip, t0);
        let c1 = lerp(va.clip, vb.clip, t1);

        let size = [self.rw as f32, self.rh as f32];
        let s0 = screen_position(c0, size);
        let s1 = screen_position(c1, size);
        /* Flat `edge_start`: provoking vertex is the last one. */
        let edge_start = screen_position(vb.clip, size);

        let (dx, dy) = (s1[0] - s0[0], s1[1] - s0[1]);
        let x_major = dx.abs() >= dy.abs();
        let (m0, m1) = if x_major { (s0[0], s1[0]) } else { (s0[1], s1[1]) };
        let (lo, hi) = (m0.min(m1), m0.max(m1));
        if hi - lo <= 0.0 {
            return;
        }
        let first = ((lo - 0.5).ceil() as i32).max(0);
        let last = ((hi - 0.5).floor() as i32).min(if x_major { self.rw } else { self.rh } - 1);

        let (inv0, inv1) = (1.0 / c0[3], 1.0 / c1[3]);
        for m in first..=last {
            let center = m as f32 + 0.5;
            let s = (center - m0) / (m1 - m0);
            let minor_pos = if x_major { s0[1] + s * dy } else { s0[0] + s * dx };
            let minor = minor_pos.floor() as i32;
            let (px, py, edge_pos) = if x_major {
                (m, minor, [center, minor_pos])
            } else {
                (minor, m, [minor_pos, center])
            };

            /* Perspective correct interpolation of the vertex outputs. */
            let u = (s * inv1) / ((1.0 - s) * inv0 + s * inv1);
            let t = t0 + u * (t1 - t0);
            let frag = GridFragIn {
                pos: lerp(va.pos, vb.pos, t),
                coord: lerp(va.coord, vb.coord, t),
                alpha: vb.alpha,
                emphasis: vb.emphasis,
                edge_start,
                edge_pos,
            };
            let z = ((c0[2] / c0[3]) * (1.0 - s) + (c1[2] / c1[3]) * s) * 0.5 + 0.5;
            self.fragment(colors, grid_flag, grid_iter, px, py, z, &frag, rv3d);
        }
    }
}

/// Draws the grid and axes of the perspective user view into `region` (inclusive Blender rect,
/// y up) of the `0x00RRGGBB` buffer. `rv3d` must have been updated for the size of the region.
pub fn draw_grid(
    buf: &mut [u32],
    width: usize,
    height: usize,
    region: Rcti,
    v3d: &View3D,
    rv3d: &RegionView3D,
    scene_depth: &[f32],
) {
    let Some(g) = grid_init_v3d(v3d, rv3d) else { return };
    let (rw, rh) = (region.size_x() + 1, region.size_y() + 1);
    if rw <= 0 || rh <= 0 {
        return;
    }
    let colors = grid_colors();
    let mut target = Target {
        buf,
        width,
        height,
        region,
        rw,
        rh,
        depth: if scene_depth.len() == (rw * rh) as usize { scene_depth.to_vec() } else { vec![1.0; (rw * rh) as usize] },
    };

    for grid_iter in 0..g.num_iters {
        for grid_flag in [g.axis_flag, g.grid_flag] {
            if grid_flag == 0 {
                continue;
            }
            for line in grid_pass_lines(&g, grid_flag, grid_iter, rv3d) {
                target.line(&colors, grid_flag, grid_iter, &line, rv3d);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_line_against_side_plane() {
        /* x runs from 0 to 3 with w = 1: it leaves the volume at x = w, a third of the way. */
        let (t0, t1) = clip_line([0.0, 0.0, 0.0, 1.0], [3.0, 0.0, 0.0, 1.0]).unwrap();
        assert_eq!(t0, 0.0);
        assert!((t1 - 1.0 / 3.0).abs() < 1e-6, "{t1}");
        assert_eq!(clip_line([2.0, 0.0, 0.0, 1.0], [3.0, 0.0, 0.0, 1.0]), None);
    }

    /* 200x100 region: the X axis is the horizontal line through the centre, drawn red-ish over the
     * background; the pixel above the grid area keeps the background colour. */
    #[test]
    fn default_view_draws_x_axis() {
        let (w, h) = (200usize, 100usize);
        let bg = crate::theme::SPACE_VIEW3D_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let v3d = View3D::default();
        let mut rv3d = RegionView3D::default();
        rv3d.update(&v3d, w as i32, h as i32);
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };
        draw_grid(&mut buf, w, h, region, &v3d, &rv3d, &vec![1.0; w * h]);

        assert!(buf.iter().any(|&p| p != bg), "nothing was drawn");
        let red = |row: usize| {
            let p = buf[row * w + 150];
            ((p >> 16) & 0xff) as i32 - ((p >> 8) & 0xff) as i32
        };
        assert!(red(49).max(red(50)) > 20, "{} {}", red(49), red(50));
    }
}
