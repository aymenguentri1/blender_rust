//! Overlay grid state for the 3D viewport (`Grid::init_v3d()`), without drawing.
//!
//! Ported from Blender:
//!  - `Grid::init_v3d()`                   (draw/engines/overlay/overlay_grid.hh:228-420)
//!  - `OVERLAY_GridBits`, `OVERLAY_GridData`, `OVERLAY_GRID_STEPS_LEN`, `OVERLAY_GRID_ITER_LEN`
//!                                         (draw/engines/overlay/overlay_shader_shared.hh:36-60, 133-156)
//!  - `view3d_grid_steps_ex()`             (editors/space_view3d/view3d_draw.cc:707)
//!  - `View3D.gridflag` default            (makesdna/DNA_view3d_types.h:775)
//!  - vertex stage `overlay_grid_vert.glsl` and `overlay_grid_common_lib.glsl`
//!                                         (draw/engines/overlay/shaders/, `grid_vertex()`)
//!  - grid colours                         (draw/engines/overlay/overlay_instance.cc:370-395)
//!  - `get_color_shade_4fv()`, `get_color_blend_shade_4fv()`
//!                                         (editors/interface/resources.cc:1375, 1407)
//!  - `srgb_to_linearrgb()`                (blenlib/intern/math_color.cc:483)
//!  - metric length units, `base_unit = 3` (blenkernel/intern/unit.cc:47-50, 296)
//!  - default scene units: metric, `scale_length = 1` (blenkernel/intern/scene.cc:223)
//!
//! Only the perspective user view (`rv3d->is_persp`, `RV3D_VIEW_USER`) of the default scene is
//! handled: no axis-aligned or orthographic views, no camera view, no XR clipping.

use crate::theme::{self, Rgba};
use crate::view3d::{Mat4, RegionView3D, View, View3D};

/* OVERLAY_GridBits. */
pub const SHOW_GRID: u32 = 1 << 0;
pub const SHOW_AXES: u32 = 1 << 1;
pub const AXIS_X: u32 = 1 << 2;
pub const AXIS_Y: u32 = 1 << 3;
#[allow(dead_code)]
pub const AXIS_Z: u32 = 1 << 4;
pub const PLANE_XY: u32 = 1 << 5;
pub const PLANE_XZ: u32 = 1 << 6;
pub const PLANE_YZ: u32 = 1 << 7;
/// Grid is in an axis-aligned view (`PLANE_XY`, ...).
pub const GRID_ALIGNED: u32 = 1 << 8;
/// Grid is shown behind geometry, on the far plane.
pub const GRID_BEHIND_GEOMETRY: u32 = 1 << 12;

/// `OVERLAY_GRID_STEPS_LEN` (= `SI_GRID_STEPS_LEN`).
pub const GRID_STEPS_LEN: usize = 8;
/// `OVERLAY_GRID_ITER_LEN`: perspective views draw the grid this many times.
pub const GRID_ITER_LEN: u32 = 4;

/* eView3D_GridFlag. */
const V3D_SHOW_FLOOR: u32 = 1 << 0;
const V3D_SHOW_X: u32 = 1 << 1;
const V3D_SHOW_Y: u32 = 1 << 2;
#[allow(dead_code)]
const V3D_SHOW_Z: u32 = 1 << 3;
const V3D_SHOW_ORTHO_GRID: u32 = 1 << 4;

/// `View3D.gridflag` default: `V3D_SHOW_X | V3D_SHOW_Y | V3D_SHOW_FLOOR | V3D_SHOW_ORTHO_GRID`.
const DEFAULT_GRIDFLAG: u32 = V3D_SHOW_X | V3D_SHOW_Y | V3D_SHOW_FLOOR | (1 << 4);

/// Metric length unit scalars in table order (km, hm, dam, m, dm, cm, mm, um); `base_unit` is 3 (meter).
const METRIC_LEN_SCALARS: [f32; 8] = [1000.0, 100.0, 10.0, 1.0, 0.1, 0.01, 0.001, 0.000_001];
const METRIC_BASE_UNIT: usize = 3;

/// The part of `OVERLAY_GridData` plus the config flags set by `Grid::init_v3d()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridState {
    pub axis_flag: u32,
    pub grid_flag: u32,
    /// `grid_ubo_.steps[i].x` (the shader gets a `float4(step)`).
    pub steps: [f32; GRID_STEPS_LEN],
    pub offset: [f32; 2],
    pub clip_rect: [f32; 2],
    pub level: f32,
    pub num_lines: u32,
    pub num_iters: u32,
}

/// `view3d_grid_steps_ex()` for the metric system; the user view skips the small units.
fn view3d_grid_steps(v3d: &View3D, user_view: bool) -> [f32; GRID_STEPS_LEN] {
    /* `grid_scale /= scene->unit.scale_length` with scale_length == 1. */
    let grid_scale = v3d.grid / 1.0;
    /* In the user view the steps are skipped: `len = BKE_unit_base_get(usys) + 1`. */
    let len = if user_view { METRIC_BASE_UNIT + 1 } else { METRIC_LEN_SCALARS.len() };

    let mut steps = [0.0f32; GRID_STEPS_LEN];
    for i in 0..len {
        steps[i] = METRIC_LEN_SCALARS[len - 1 - i] * grid_scale;
    }
    /* Fill last slots. */
    for i in len..GRID_STEPS_LEN {
        steps[i] = steps[len - 1];
    }
    steps
}

#[inline]
fn safe_divide(a: f32, b: f32) -> f32 {
    if b != 0.0 {
        a / b
    } else {
        0.0
    }
}

/// `Grid::init_v3d()` for a perspective user view with the default `gridflag`.
/// `rv3d` must have been updated (`viewinv`).
pub fn grid_init_v3d(v3d: &View3D, rv3d: &RegionView3D) -> Option<GridState> {
    let gridflag = DEFAULT_GRIDFLAG;
    let show_axis_x = gridflag & V3D_SHOW_X != 0;
    let show_axis_y = gridflag & V3D_SHOW_Y != 0;
    let show_persp = gridflag & V3D_SHOW_FLOOR != 0;

    /* Perspective/user views show the selected axes and the floor (XY) plane; axis-aligned
     * orthographic views show the axes and the plane that faces the view. */
    let aligned = rv3d.ortho && rv3d.view != View::User;
    let show_axis_z = gridflag & V3D_SHOW_Z != 0;
    let show_ortho = gridflag & V3D_SHOW_ORTHO_GRID != 0;
    let mut axis_flag: u32 = 0;
    let mut grid_flag: u32 = 0;
    if !aligned {
        if show_axis_x {
            axis_flag |= AXIS_X;
        }
        if show_axis_y {
            axis_flag |= AXIS_Y;
        }
        if show_persp {
            grid_flag |= PLANE_XY;
        }
        if axis_flag != 0 {
            axis_flag |= SHOW_AXES;
        }
        if grid_flag != 0 {
            grid_flag |= SHOW_GRID;
        }
        /* Axes are passed to the grid flag for correct occlusion. */
        if grid_flag != 0 {
            if show_axis_x {
                grid_flag |= AXIS_X;
            }
            if show_axis_y {
                grid_flag |= AXIS_Y;
            }
        }
    } else {
        let bit = |show: bool, flag: u32| if show { flag | GRID_ALIGNED } else { 0 };
        match rv3d.view {
            View::Right | View::Left => {
                axis_flag = bit(show_axis_y, AXIS_Y) | bit(show_axis_z, AXIS_Z);
                grid_flag = bit(show_ortho, PLANE_YZ);
            }
            View::Top | View::Bottom => {
                axis_flag = bit(show_axis_x, AXIS_X) | bit(show_axis_y, AXIS_Y);
                grid_flag = bit(show_ortho, PLANE_XY);
            }
            View::Front | View::Back => {
                axis_flag = bit(show_axis_x, AXIS_X) | bit(show_axis_z, AXIS_Z);
                grid_flag = bit(show_ortho, PLANE_XZ);
            }
            View::User => {}
        }
        /* Fixed plane views place the grid on the far plane. */
        if axis_flag != 0 {
            axis_flag |= SHOW_AXES | GRID_BEHIND_GEOMETRY;
        }
        if grid_flag != 0 {
            grid_flag |= SHOW_GRID | GRID_BEHIND_GEOMETRY;
        }
        /* Axes are passed to the grid flag for correct occlusion. */
        if grid_flag != 0 {
            if show_axis_x {
                grid_flag |= AXIS_X;
            }
            if show_axis_y {
                grid_flag |= AXIS_Y;
            }
            if show_axis_z {
                grid_flag |= AXIS_Z;
            }
        }
    }
    if grid_flag == 0 && axis_flag == 0 {
        return None;
    }

    /* Grid scales: a level equal to the previous one is the previous level times 10. */
    let base = view3d_grid_steps(v3d, rv3d.view == View::User);
    let mut steps = [0.0f32; GRID_STEPS_LEN];
    for i in 0..GRID_STEPS_LEN {
        steps[i] = if i > 0 && (base[i] == 0.0 || base[i] == base[i - 1]) {
            steps[i - 1] * 10.0
        } else {
            base[i]
        };
    }

    /* Camera parameters: `viewinv[3]` is the position, `viewinv[2]` the (backward) axis. */
    let pos = [rv3d.viewinv[3][0], rv3d.viewinv[3][1], rv3d.viewinv[3][2]];
    let fwd = [rv3d.viewinv[2][0], rv3d.viewinv[2][1], rv3d.viewinv[2][2]];

    /* Distance to a floor point of interest: interpolate between the viewed point and the point
     * directly below the camera, depending on azimuth. */
    let aligned_grid = (grid_flag | axis_flag) & GRID_ALIGNED != 0;
    let dist = if !rv3d.ortho {
        let a = (pos[2] / fwd[2]).abs();
        let b = pos[2].abs();
        let t = 1.0 - fwd[2].abs();
        a * (1.0 - t) + b * t
    } else if aligned_grid {
        /* Same zoom-derived distance as `ED_view3d_grid_view_scale()`, kept from going below the
         * smallest available grid step. */
        let d = 10.0 * 12.0 / (rv3d.winx as f32 * rv3d.winmat[0][0]);
        d.max(steps[0])
    } else {
        /* Orthographic: the scale is simply the view distance. */
        rv3d.dist
    };

    /* Grid offset: the point `dist` along the view direction, on the floor plane. */
    let offset = match rv3d.view {
        View::Right | View::Left => [pos[1], pos[2]],
        View::Top | View::Bottom => [pos[0], pos[1]],
        View::Front | View::Back => [pos[0], pos[2]],
        View::User if !rv3d.ortho => [pos[0] - dist * fwd[0], pos[1] - dist * fwd[1]],
        /* Orthographic: the intersection of the view ray through `viewinv[3]` with the XY plane. */
        View::User => {
            if fwd[2].abs() > 1e-6 {
                let k = safe_divide(pos[2], fwd[2]);
                [pos[0] - fwd[0] * k, pos[1] - fwd[1] * k]
            } else {
                [pos[0], pos[1]]
            }
        }
    };

    /* Lowest relevant grid level for that distance. */
    let mut level = 0.0;
    for i in 0..GRID_STEPS_LEN {
        let curr = steps[i];
        let next = if i < GRID_STEPS_LEN - 1 { steps[i + 1] } else { curr * 10.0 };
        if next >= dist || i == GRID_STEPS_LEN - 1 {
            level = i as f32 + safe_divide(dist - curr, next - curr);
            break;
        }
    }

    /* This suffices for most cases, and in others we fade to hide it. */
    let (num_lines, num_iters) = if !rv3d.ortho {
        /* Perspective views draw a fixed number of lines, N times at different Z. */
        (151u32, GRID_ITER_LEN)
    } else if aligned_grid {
        /* The practical size of the viewport in line steps, at most 8192 lines per axis. */
        let lowest_step = steps[(level as usize).min(GRID_STEPS_LEN - 1)];
        let lines_x = 2 * (((1.0 / rv3d.winmat[0][0]) / lowest_step).ceil() as u32) + 1;
        let lines_y = 2 * (((1.0 / rv3d.winmat[1][1]) / lowest_step).ceil() as u32) + 1;
        (lines_x.max(lines_y).min(8192), 1)
    } else {
        /* Other orthographic views draw a fixed number of lines once. */
        (301, 1)
    };
    Some(GridState {
        axis_flag,
        grid_flag,
        steps,
        offset,
        /* `use_clip_end` is true for perspective views. */
        clip_rect: if rv3d.ortho {
            let r = (4.0 / rv3d.winmat[0][0]).max(4.0 / rv3d.winmat[1][1]);
            [r, r]
        } else {
            [v3d.clip_end, v3d.clip_end]
        },
        level,
        num_lines,
        num_iters,
    })
}

/// `name_display` of the metric length units in table order (blenkernel/intern/unit.cc:177-296; the
/// nanometer and picometer entries are under `#if 0`).
const METRIC_LEN_DISPLAY_NAMES: [&str; 8] = [
    "Kilometers",
    "100 Meters",
    "10 Meters",
    "Meters",
    "10 Centimeters",
    "Centimeters",
    "Millimeters",
    "Micrometers",
];

/// The unit name that `ED_view3d_grid_view_scale()` (editors/space_view3d/view3d_draw.cc:1186-1225) returns
/// for an axis-aligned orthographic view: the first grid step whose next step reaches the distance
/// `10 * 12 / (winx * winmat[0][0])`, named by `BKE_unit_display_name_get(usys, len - i - 1)`. `None` in the
/// perspective and user views, where `draw_grid_unit_name()` draws nothing. `rv3d` must have been updated.
pub fn grid_unit_name(v3d: &View3D, rv3d: &RegionView3D) -> Option<&'static str> {
    if !rv3d.ortho || rv3d.view == View::User {
        return None;
    }
    let dist = 10.0 * 12.0 / (rv3d.winx as f32 * rv3d.winmat[0][0]);
    let steps = view3d_grid_steps(v3d, false);
    let mut i = 0;
    while i < GRID_STEPS_LEN - 1 && steps[i + 1] < dist {
        i += 1;
    }
    Some(METRIC_LEN_DISPLAY_NAMES[GRID_STEPS_LEN - 1 - i])
}


/// The overlay grid colours (`gb.colors.grid`, `grid_emphasis`, `grid_axis_x/y/z`) as linear RGBA.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridColors {
    pub grid: [f32; 4],
    pub grid_emphasis: [f32; 4],
    pub axis_x: [f32; 4],
    pub axis_y: [f32; 4],
    pub axis_z: [f32; 4],
}

fn rgba_to_float(c: Rgba) -> [f32; 4] {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0]
}

/// `get_color_shade_4fv()`: the offset is added to RGB in 8 bit, alpha is kept.
fn color_shade_4fv(c: Rgba, offset: i32) -> [f32; 4] {
    let ch = |v: u8| (offset + v as i32).clamp(0, 255) as f32 / 255.0;
    [ch(c.r), ch(c.g), ch(c.b), c.a as f32 / 255.0]
}

/// `get_color_blend_shade_4fv()`: the offset applies to RGB only.
fn color_blend_shade_4fv(c1: Rgba, c2: Rgba, fac: f32, offset: i32) -> [f32; 4] {
    let fac = fac.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| ((1.0 - fac) * a as f32 + fac * b as f32).floor() as i32;
    let ch = |a: u8, b: u8| (offset + mix(a, b)).clamp(0, 255) as f32 / 255.0;
    [
        ch(c1.r, c2.r),
        ch(c1.g, c2.g),
        ch(c1.b, c2.b),
        mix(c1.a, c2.a).clamp(0, 255) as f32 / 255.0,
    ]
}

/// `srgb_to_linearrgb()`.
pub fn srgb_to_linearrgb(c: f32) -> f32 {
    if c < 0.04045 {
        return if c < 0.0 { 0.0 } else { c * (1.0 / 12.92) };
    }
    ((c + 0.055) * (1.0 / 1.055)).powf(2.4)
}

/// `srgb_to_linearrgb_v4()`: RGB is converted, alpha is kept.
fn srgb_to_linearrgb_v4(c: [f32; 4]) -> [f32; 4] {
    [srgb_to_linearrgb(c[0]), srgb_to_linearrgb(c[1]), srgb_to_linearrgb(c[2]), c[3]]
}

/// Grid colours as set up in `overlay_instance.cc` for a 3D viewport (`state.rv3d != nullptr`).
pub fn grid_colors() -> GridColors {
    let background = rgba_to_float(theme::SPACE_VIEW3D_BACK);

    let grid = color_shade_4fv(theme::SPACE_VIEW3D_GRID, 10);
    /* Emphasize division lines lighter instead of darker, if background is darker than grid. */
    let is_bg_darker = (grid[0] + grid[1] + grid[2]) + 0.12 > (background[0] + background[1] + background[2]);
    let grid_emphasis = color_shade_4fv(theme::SPACE_VIEW3D_GRID_MAJOR, if is_bg_darker { 20 } else { -10 });

    let grid_axis_offset = ((theme::SPACE_VIEW3D_GRID_AXIS_BRIGHTNESS * 2.0 - 1.0) * 255.0) as i32;
    let axis = |c: Rgba| color_blend_shade_4fv(theme::SPACE_VIEW3D_GRID, c, 0.85, grid_axis_offset);

    /* Color management. */
    GridColors {
        grid: srgb_to_linearrgb_v4(grid),
        grid_emphasis: srgb_to_linearrgb_v4(grid_emphasis),
        axis_x: srgb_to_linearrgb_v4(axis(theme::TUI_XAXIS)),
        axis_y: srgb_to_linearrgb_v4(axis(theme::TUI_YAXIS)),
        axis_z: srgb_to_linearrgb_v4(axis(theme::TUI_ZAXIS)),
    }
}


/// `OVERLAY_GRID_STEPS_DRAW`: grid levels drawn at a time.
const GRID_STEPS_DRAW: u32 = 3;

/// The vertex stage outputs that matter for rasterisation: world position, `vertex_out.coord`,
/// the flat `alpha` / `emphasis`, and `gl_Position`. (`edge_start` / `edge_pos`, used for the
/// fragment stage's line anti-aliasing, are derived from `clip` and the viewport size.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridVertex {
    pub pos: [f32; 3],
    pub coord: [f32; 2],
    pub alpha: f32,
    pub emphasis: f32,
    pub clip: [f32; 4],
}

#[inline]
fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// `winmat * (viewmat * v)` for column-major matrices.
fn mul_m4_v4(m: &Mat4, v: [f32; 4]) -> [f32; 4] {
    let mut r = [0.0f32; 4];
    for i in 0..4 {
        r[i] = m[0][i] * v[0] + m[1][i] * v[1] + m[2][i] * v[2] + m[3][i] * v[3];
    }
    r
}

/// Vertex stage of `overlay_grid_vert.glsl` (`main()`) for a perspective view.
///
/// `grid_flag` is the push constant of the pass (`axis_flag` for the axis pass with 6 vertices,
/// `grid_flag` for the grid pass with `4 * GRID_STEPS_DRAW * num_lines` vertices) and `grid_iter`
/// the draw iteration. Returns `None` where the shader discards the line (`gl_Position = NaN` or
/// an early return). Only the XY plane is handled.
pub fn grid_vertex(
    g: &GridState,
    grid_flag: u32,
    grid_iter: u32,
    vertex_id: u32,
    rv3d: &RegionView3D,
) -> Option<GridVertex> {
    let show_grid = grid_flag & SHOW_GRID != 0;
    let lines_half = ((g.num_lines >> 1) as f32).max(1.0);

    /* decode_grid_data() / decode_axis_data(): P, axis, level. */
    let (mut p, axis, line_level): ([f32; 2], usize, u32) = if show_grid {
        /* Every pair of consecutive verts forms a line (bit 0), every pair of lines flips x/y
         * (bit 1); the remaining bits are the level (large to small) and the line index. */
        let side = vertex_id & 1;
        let mut id = vertex_id >> 1;
        let axis = id & 1;
        id >>= 1;
        let level = (GRID_STEPS_DRAW - 1).checked_sub(id / g.num_lines)?;
        id %= g.num_lines;
        let line_index = (id as f32 - lines_half) / lines_half;
        let (x, y) = (line_index, if side != 0 { 1.0 } else { -1.0 });
        (if axis == 0 { [x, y] } else { [y, x] }, axis as usize, level)
    } else {
        let side = vertex_id & 1;
        let axis = vertex_id >> 1;
        ([if side != 0 { 1.0 } else { -1.0 }, 0.0], axis as usize, GRID_STEPS_DRAW - 1)
    };
    if axis > 2 {
        return None;
    }

    /* Actual level of the line, offset by -1 to force a sub-level in the 3D viewport. */
    let level_offset = if grid_flag & GRID_ALIGNED != 0 { 0 } else { 1 };
    let level = ((g.level as i32) + (line_level as i32) - level_offset).clamp(0, GRID_STEPS_LEN as i32 - 1) as usize;

    let step_size = g.steps[level];
    if step_size == 0.0 {
        return None;
    }

    /* Per-level camera offset: rounded for the grid, following the camera for axes. */
    let mut step_offs = g.offset;
    if show_grid {
        if axis != 0 {
            step_offs[1] = (step_offs[1] / step_size).round() * step_size;
        } else {
            step_offs[0] = (step_offs[0] / step_size).round() * step_size;
        }
    } else if grid_flag & SHOW_AXES != 0 {
        step_offs = [rv3d.viewinv[3][axis], 0.0];
    }

    let coord = p;
    let fract_level = g.level - g.level.floor();
    let emphasis = saturate(line_level as f32 - fract_level);
    let mut alpha = saturate(line_level as f32 + 1.0 - fract_level);
    if rv3d.ortho {
        /* Also fade by pixel size for orthographic, as we lack proper line DFDX/DFDY. */
        alpha *= smoothstep(step_size * 0.25, step_size * 0.25 * 0.25 * 0.25, rv3d.pixsize);
    }

    /* Scale grid lines to [-N, N] and offset them in level-appropriate steps. */
    p = [step_offs[0] + step_size * (p[0] * lines_half), step_offs[1] + step_size * (p[1] * lines_half)];

    /* Clipping rectangle. */
    let (clip_min, clip_max) = if show_grid {
        (
            [step_offs[0] - g.clip_rect[0], step_offs[1] - g.clip_rect[1]],
            [step_offs[0] + g.clip_rect[0], step_offs[1] + g.clip_rect[1]],
        )
    } else {
        /* unpack_xy_to_axis(): the Z axis uses the x value; clipping is on the x component. */
        let clip = if axis == 2 { g.clip_rect[0] } else { g.clip_rect[axis] };
        ([step_offs[0] - clip, 0.0], [step_offs[0] + clip, 0.0])
    };
    let outside = (p[0] < clip_min[0] && p[1] < clip_min[1]) || (p[0] > clip_max[0] && p[1] > clip_max[1]);
    if outside {
        return None;
    }
    p = [p[0].clamp(clip_min[0], clip_max[0]), p[1].clamp(clip_min[1], clip_max[1])];

    /* World-space position. */
    let mut pos = [0.0f32; 3];
    if show_grid {
        if grid_flag & PLANE_XY != 0 {
            pos[0] = p[0];
            pos[1] = p[1];
        } else if grid_flag & PLANE_XZ != 0 {
            pos[0] = p[0];
            pos[2] = p[1];
        } else if grid_flag & PLANE_YZ != 0 {
            pos[1] = p[0];
            pos[2] = p[1];
        } else {
            return None;
        }
    } else {
        let axis_bit = [AXIS_X, AXIS_Y, AXIS_Z][axis];
        if grid_flag & axis_bit == 0 {
            return None;
        }
        pos[axis] = p[0];
    }

    /* is_occluded_by_higher_level(). */
    if show_grid && line_level < GRID_STEPS_DRAW - 1 && level < GRID_STEPS_LEN - 1 {
        let j = p[axis].abs() / g.steps[level + 1];
        if (j.floor() - j).abs() <= 1e-4 {
            return None;
        }
    }

    let view = mul_m4_v4(&rv3d.viewmat, [pos[0], pos[1], pos[2], 1.0]);
    let mut clip = mul_m4_v4(&rv3d.winmat, view);

    /* Progressive z-bias so the grid fades through geometry. */
    const Z_MIN_OFFSET: f32 = -0.00020;
    const Z_MAX_OFFSET: f32 = 0.00025;
    let z_factor = (grid_iter * GRID_STEPS_DRAW + line_level) as f32 / (GRID_ITER_LEN * GRID_STEPS_DRAW) as f32;
    if !rv3d.ortho {
        clip[2] += Z_MAX_OFFSET * (1.0 - z_factor) + Z_MIN_OFFSET * z_factor;
    } else if grid_flag & GRID_BEHIND_GEOMETRY != 0 {
        /* Set z to the far plane, placing the grid behind all geometry. */
        clip[2] = 1.0;
    }

    Some(GridVertex { pos, coord, alpha, emphasis, clip })
}


/// The fragment stage inputs of `overlay_grid_frag.glsl`: `vertex_out` (`pos`, `coord`, perspective
/// interpolated), `vertex_out_flat` (`alpha`, `emphasis`), `edge_start` (flat) and `edge_pos`
/// (no perspective). `edge_start` / `edge_pos` are `screen_position()` of the line's first vertex
/// and of the fragment, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridFragIn {
    pub pos: [f32; 3],
    pub coord: [f32; 2],
    pub alpha: f32,
    pub emphasis: f32,
    pub edge_start: [f32; 2],
    pub edge_pos: [f32; 2],
}

/// `overlay_grid_vert.glsl` `screen_position()`: clip position to viewport pixels.
pub fn screen_position(clip: [f32; 4], size_viewport: [f32; 2]) -> [f32; 2] {
    [
        ((clip[0] / clip[3]) * 0.5 + 0.5) * size_viewport[0],
        ((clip[1] / clip[3]) * 0.5 + 0.5) * size_viewport[1],
    ]
}

/// GLSL `smoothstep()`.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = saturate((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

/// Grid iteration additive alpha in perspective views (`additive_alpha[OVERLAY_GRID_ITER_LEN]`).
const ADDITIVE_ALPHA: [f32; GRID_ITER_LEN as usize] = [1.0, 0.50, 0.25, 0.125];

/// Fragment stage of `overlay_grid_frag.glsl` (`main()`) for a perspective view: returns
/// `out_color`, or `None` where the shader discards the fragment. `grid_iter` must be below
/// `GRID_ITER_LEN`. The anti-aliasing output `line_output` is not produced.
pub fn grid_fragment(
    colors: &GridColors,
    grid_flag: u32,
    grid_iter: u32,
    f: &GridFragIn,
    rv3d: &RegionView3D,
) -> Option<[f32; 4]> {
    /* Test if a vertex output position overlaps with an active axis line. */
    const AXIS_EPSILON: f32 = 2e-7;
    let near_zero = |a: f32, b: f32| a.abs() <= AXIS_EPSILON && b.abs() <= AXIS_EPSILON;
    let axis_x = grid_flag & AXIS_X != 0 && near_zero(f.pos[1], f.pos[2]);
    let axis_y = grid_flag & AXIS_Y != 0 && near_zero(f.pos[0], f.pos[2]);
    let axis_z = grid_flag & AXIS_Z != 0 && near_zero(f.pos[0], f.pos[1]);

    /* If an axis line overlaps, the fragment can be discarded. */
    if (axis_x || axis_y || axis_z) && grid_flag & SHOW_GRID != 0 {
        return None;
    }

    /* Axis colour is fixed by theme, the grid colour is a mix of [grid, grid_emphasis]. */
    let mut color = if axis_x {
        colors.axis_x
    } else if axis_y {
        colors.axis_y
    } else if axis_z {
        colors.axis_z
    } else {
        let mut mixed = [0.0f32; 4];
        for i in 0..4 {
            mixed[i] = colors.grid[i] * (1.0 - f.emphasis) + colors.grid_emphasis[i] * f.emphasis;
        }
        mixed
    };

    /* Fragment alpha. */
    color[3] *= f.alpha;

    if !rv3d.ortho {
        /* Fade at edge of grid level. */
        let length_fade = 1.0 - (f.coord[0] * f.coord[0] + f.coord[1] * f.coord[1]).sqrt().min(1.0);
        color[3] *= length_fade;

        /* Compute normalized view vector (`drw_view_position()` is `viewinv[3]`). */
        let mut v = [
            rv3d.viewinv[3][0] - f.pos[0],
            rv3d.viewinv[3][1] - f.pos[1],
            rv3d.viewinv[3][2] - f.pos[2],
        ];
        let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        for c in &mut v {
            *c /= dist;
        }

        /* Add fade at steep angles for contents of the floor plane. */
        if f.pos[2] == 0.0 {
            let t = 1.0 - v[2].abs();
            color[3] *= 1.0 - t * t * t;
        }

        /* Add fade towards camera clip plane: `far_clip = -drw_view_far()`. */
        let far_clip = rv3d.winmat[3][2] / (rv3d.winmat[2][2] + 1.0);
        color[3] *= 1.0 - smoothstep(0.0, 0.5 * far_clip, dist - 0.5 * far_clip);
    } else {
        /* Fade at edge of grid level in orthographic, in case of rather small units. */
        if grid_flag & GRID_ALIGNED == 0 {
            let length_fade = 1.0 - (f.coord[0] * f.coord[0] + f.coord[1] * f.coord[1]).min(1.0);
            color[3] *= length_fade * length_fade;
        }

        /* Add fade at steep angles for contents of the floor plane (`V = -drw_view_forward()`). */
        if grid_flag & PLANE_XY != 0 {
            let t = 1.0 - rv3d.viewinv[2][2].abs();
            color[3] *= 1.0 - t * t * t;
        }
    }

    /* Alpha discard by stipple pattern for low alpha. */
    const DASH_WIDTH: f32 = 4.0;
    const FADE_START: f32 = 0.1;
    let edge_dist = ((f.edge_start[0] - f.edge_pos[0]).powi(2) + (f.edge_start[1] - f.edge_pos[1]).powi(2)).sqrt();
    let t = edge_dist / DASH_WIDTH;
    if color[3] < FADE_START && (1.0 / FADE_START) * color[3] < t - t.floor() {
        return None;
    }

    /* Grid iteration additive alpha in perspective view. */
    if !rv3d.ortho {
        color[3] *= ADDITIVE_ALPHA[grid_iter as usize];
    }
    Some(color)
}


/// Primitive assembly of one `draw_procedural(GPU_PRIM_LINES, ...)` call in `Grid::begin_sync()`
/// (overlay_grid.hh:94-103): the axis pass (`grid_flag` without `SHOW_GRID`) draws 6 vertices, the
/// grid pass draws `4 * OVERLAY_GRID_STEPS_DRAW * num_lines`. Vertices `2k` and `2k + 1` form a
/// line; a line is dropped when either vertex was discarded by the vertex stage (NaN position).
pub fn grid_pass_lines(
    g: &GridState,
    grid_flag: u32,
    grid_iter: u32,
    rv3d: &RegionView3D,
) -> Vec<[GridVertex; 2]> {
    let vertex_count = if grid_flag & SHOW_GRID != 0 { 4 * GRID_STEPS_DRAW * g.num_lines } else { 6 };
    let mut lines = Vec::new();
    for id in (0..vertex_count).step_by(2) {
        if let (Some(a), Some(b)) = (
            grid_vertex(g, grid_flag, grid_iter, id, rv3d),
            grid_vertex(g, grid_flag, grid_iter, id + 1, rv3d),
        ) {
            lines.push([a, b]);
        }
    }
    lines
}


#[cfg(test)]
mod tests {
    use super::*;

    /* Default view: camera 10 units back on +Z looking at the origin, so dist = 10 sits exactly on
     * the second step and the level is 1.0; the grid is centred on the origin. */
    #[test]
    fn default_view_grid() {
        let v3d = View3D::default();
        let mut rv3d = RegionView3D::default();
        rv3d.update(&v3d, 1920, 1000);
        let g = grid_init_v3d(&v3d, &rv3d).expect("grid enabled by default");

        assert_eq!(g.axis_flag, SHOW_AXES | AXIS_X | AXIS_Y);
        assert_eq!(g.grid_flag, SHOW_GRID | PLANE_XY | AXIS_X | AXIS_Y);
        assert_eq!(g.steps, [1.0, 10.0, 100.0, 1000.0, 10000.0, 100000.0, 1000000.0, 10000000.0]);
        assert!(g.offset[0].abs() < 1e-5 && g.offset[1].abs() < 1e-5, "{:?}", g.offset);
        assert_eq!(g.clip_rect, [1000.0, 1000.0]);
        assert!((g.level - 1.0).abs() < 1e-5, "{}", g.level);
        assert_eq!(g.num_lines, 151);
        assert_eq!(g.num_iters, 4);
    }

    /* Theme grid 0x545454 + 10 = 94, alpha 0x80; the background (0x3d) is darker than the grid so
     * the emphasis is grid_major (0x54) + 20 = 104 with alpha 255. The offset for the axes is
     * int((0.46 * 2 - 1) * 255) = -20: X is floor(0.15 * 84 + 0.85 * 255) - 20 = 209, alpha
     * floor(0.15 * 128 + 0.85 * 255) = 235. RGB goes to linear, alpha does not. */
    #[test]
    fn default_grid_colors() {
        let c = grid_colors();
        assert_eq!(c.grid, [srgb_to_linearrgb(94.0 / 255.0); 3].iter().copied().chain([128.0 / 255.0]).collect::<Vec<_>>()[..]);
        assert_eq!(c.grid_emphasis[0], srgb_to_linearrgb(104.0 / 255.0));
        assert_eq!(c.grid_emphasis[3], 1.0);
        assert_eq!(c.axis_x[0], srgb_to_linearrgb(209.0 / 255.0));
        assert_eq!(c.axis_x[3], 235.0 / 255.0);
        assert!(c.axis_x[0] > c.axis_x[1] && c.axis_y[1] > c.axis_y[2] && c.axis_z[2] > c.axis_z[0]);
    }

    #[test]
    fn srgb_to_linear_endpoints() {
        assert_eq!(srgb_to_linearrgb(0.0), 0.0);
        assert!((srgb_to_linearrgb(1.0) - 1.0).abs() < 1e-6);
        assert!((srgb_to_linearrgb(0.02) - 0.02 / 12.92).abs() < 1e-9);
    }
    /* Orthographic default view, 1920x1000: the grid is centred on the origin, the distance is the
     * view distance (level 1.0), 301 lines are drawn once and the clip rectangle is
     * max(4 / winmat[0][0], 4 / winmat[1][1]) = 28.8. */
    #[test]
    fn ortho_grid_state() {
        let v3d = View3D::default();
        let mut rv3d = RegionView3D::default();
        rv3d.toggle_persp_ortho();
        rv3d.update(&v3d, 1920, 1000);
        let g = grid_init_v3d(&v3d, &rv3d).unwrap();
        assert!(g.offset[0].abs() < 1e-5 && g.offset[1].abs() < 1e-5, "{:?}", g.offset);
        assert!((g.level - 1.0).abs() < 1e-5, "{}", g.level);
        assert_eq!((g.num_lines, g.num_iters), (301, 1));
        assert!((g.clip_rect[0] - 28.8).abs() < 1e-3, "{:?}", g.clip_rect);
    }
    /* Top view, 1920x1000: the XY plane with the X and Y axes, on the far plane; the distance is
     * 120 / (1920 * winmat[0][0]) = 0.45 with the unit steps 1e-6 ... 1000, so the level is
     * 3 + (0.45 - 0.1) / (1 - 0.1). */
    #[test]
    fn top_view_grid_state() {
        let v3d = View3D::default();
        let mut rv3d = RegionView3D::default();
        rv3d.view_axis(View::Top);
        rv3d.update(&v3d, 1920, 1000);
        let g = grid_init_v3d(&v3d, &rv3d).unwrap();
        assert_eq!(g.axis_flag, SHOW_AXES | GRID_BEHIND_GEOMETRY | GRID_ALIGNED | AXIS_X | AXIS_Y);
        assert_eq!(g.grid_flag, SHOW_GRID | GRID_BEHIND_GEOMETRY | GRID_ALIGNED | PLANE_XY | AXIS_X | AXIS_Y);
        assert!((g.steps[0] - 1e-6).abs() < 1e-12 && g.steps[7] == 1000.0, "{:?}", g.steps);
        assert!(g.offset[0].abs() < 1e-5 && g.offset[1].abs() < 1e-5, "{:?}", g.offset);
        assert!((g.level - 3.388_889).abs() < 1e-3, "{}", g.level);
        assert!((145..=147).contains(&g.num_lines), "{}", g.num_lines);
        assert_eq!(g.num_iters, 1);
    }
    /* Top view, 1920x1000: the distance is 0.45, so the steps 1e-6, 0.001, 0.01, 0.1 are below it and
     * the next one (1) reaches it: step index 3 (0.1 m), named by unit index 7 - 3 = 4. The perspective
     * view has no unit name. */
    #[test]
    fn grid_unit_name_in_axis_views() {
        let v3d = View3D::default();
        let mut rv3d = RegionView3D::default();
        rv3d.update(&v3d, 1920, 1000);
        assert_eq!(grid_unit_name(&v3d, &rv3d), None);
        rv3d.view_axis(View::Top);
        rv3d.update(&v3d, 1920, 1000);
        assert_eq!(grid_unit_name(&v3d, &rv3d), Some("10 Centimeters"));
    }
    fn default_grid() -> (GridState, RegionView3D) {
        let v3d = View3D::default();
        let mut rv3d = RegionView3D::default();
        rv3d.update(&v3d, 1920, 1000);
        (grid_init_v3d(&v3d, &rv3d).unwrap(), rv3d)
    }

    /* Vertex 1512 is the first end of the line with index 76 of the smallest level: x = 1 / 75 * 75
     * (step 1), spanning y = -75. The level-2 lines of vertex 0 lie outside the clip rectangle. */
    #[test]
    fn vertex_stage_grid_line() {
        let (g, rv3d) = default_grid();
        assert_eq!(grid_vertex(&g, g.grid_flag, 0, 0, &rv3d), None);

        let v = grid_vertex(&g, g.grid_flag, 0, 1512, &rv3d).expect("visible line");
        assert!((v.pos[0] - 1.0).abs() < 1e-5 && (v.pos[1] + 75.0).abs() < 1e-4 && v.pos[2] == 0.0, "{:?}", v.pos);
        assert!((v.coord[0] - 1.0 / 75.0).abs() < 1e-6 && v.coord[1] == -1.0, "{:?}", v.coord);
        assert_eq!((v.alpha, v.emphasis), (1.0, 0.0));
        /* Camera at +10 on Z looking down: w = 10, x = 1.388889 * 1. */
        assert!((v.clip[3] - 10.0).abs() < 1e-4, "{:?}", v.clip);
        assert!((v.clip[0] - 1.388_889).abs() < 1e-3, "{:?}", v.clip);
    }

    /* The centre line (index 75) falls under a line of the next level and is discarded. */
    #[test]
    fn vertex_stage_discards_occluded_line() {
        let (g, rv3d) = default_grid();
        assert_eq!(grid_vertex(&g, g.grid_flag, 0, 1508, &rv3d), None);
    }

    /* Axis pass: vertex 0 is the -X end of the X axis, following the camera (x = 0) at +-1000. */
    #[test]
    fn vertex_stage_axis_line() {
        let (g, rv3d) = default_grid();
        let v = grid_vertex(&g, g.axis_flag, 0, 1, &rv3d).expect("X axis");
        assert!((v.pos[0] - 1000.0).abs() < 1e-3 && v.pos[1] == 0.0 && v.pos[2] == 0.0, "{:?}", v.pos);
        assert_eq!(grid_vertex(&g, g.axis_flag, 0, 4, &rv3d), None, "Z axis is not enabled");
    }
    /* Zooming out to dist = 55 lies halfway between the 10 and 100 steps: level 1 + 45/90. */
    #[test]
    fn level_is_fractional_between_steps() {
        let v3d = View3D::default();
        let mut rv3d = RegionView3D::default();
        rv3d.dist = 55.0;
        rv3d.update(&v3d, 1920, 1000);
        let g = grid_init_v3d(&v3d, &rv3d).unwrap();
        assert!((g.level - 1.5).abs() < 1e-4, "{}", g.level);
    }

    fn frag_at(pos: [f32; 3]) -> GridFragIn {
        GridFragIn {
            pos,
            coord: [0.0, 0.0],
            alpha: 1.0,
            emphasis: 0.0,
            edge_start: [0.0, 0.0],
            edge_pos: [0.0, 0.0],
        }
    }

    /* Directly below the camera (dist 10, V.z = 1) nothing fades: the output is the grid colour. */
    #[test]
    fn fragment_grid_colour() {
        let (_, rv3d) = default_grid();
        let c = grid_colors();
        let out = grid_fragment(&c, SHOW_GRID | PLANE_XY, 0, &frag_at([0.0; 3]), &rv3d).expect("visible");
        for i in 0..4 {
            assert!((out[i] - c.grid[i]).abs() < 1e-4, "[{i}] {} vs {}", out[i], c.grid[i]);
        }
    }

    /* Emphasis 1 selects `grid_emphasis`; iteration 2 scales the alpha by 0.25. */
    #[test]
    fn fragment_emphasis_and_additive_alpha() {
        let (_, rv3d) = default_grid();
        let c = grid_colors();
        let mut f = frag_at([0.0; 3]);
        f.emphasis = 1.0;
        let out = grid_fragment(&c, SHOW_GRID | PLANE_XY, 2, &f, &rv3d).expect("visible");
        assert!((out[0] - c.grid_emphasis[0]).abs() < 1e-4);
        assert!((out[3] - 0.25 * c.grid_emphasis[3]).abs() < 1e-4, "{}", out[3]);
    }

    /* A grid fragment on an enabled axis is discarded (the axis pass draws it); the axis pass
     * itself outputs the axis colour. */
    #[test]
    fn fragment_axis_overlap() {
        let (g, rv3d) = default_grid();
        let c = grid_colors();
        assert_eq!(grid_fragment(&c, g.grid_flag, 0, &frag_at([3.0, 0.0, 0.0]), &rv3d), None);
        let out = grid_fragment(&c, g.axis_flag, 0, &frag_at([3.0, 0.0, 0.0]), &rv3d).expect("axis");
        assert!((out[0] - c.axis_x[0]).abs() < 1e-6 && (out[1] - c.axis_x[1]).abs() < 1e-6);
        let out = grid_fragment(&c, g.axis_flag, 0, &frag_at([0.0, 3.0, 0.0]), &rv3d).expect("axis");
        assert!((out[1] - c.axis_y[1]).abs() < 1e-6);
    }

    /* Alpha below 0.1 is stippled along the line: kept where `10 * alpha >= fract(dist / 4)`. */
    #[test]
    fn fragment_dash_discard() {
        let (g, rv3d) = default_grid();
        let c = grid_colors();
        let mut f = frag_at([5.0, 0.0, 0.0]);
        f.alpha = 0.05;
        f.edge_pos = [3.0, 0.0];
        assert_eq!(grid_fragment(&c, g.axis_flag, 0, &f, &rv3d), None);
        f.edge_pos = [1.0, 0.0];
        assert!(grid_fragment(&c, g.axis_flag, 0, &f, &rv3d).is_some());
    }

    /* The axis pass has the X and Y lines (Z is not enabled): both span +-1000 around the camera. */
    #[test]
    fn axis_pass_lines() {
        let (g, rv3d) = default_grid();
        let lines = grid_pass_lines(&g, g.axis_flag, 0, &rv3d);
        assert_eq!(lines.len(), 2);
        assert!((lines[0][0].pos[0] + 1000.0).abs() < 1e-3 && (lines[0][1].pos[0] - 1000.0).abs() < 1e-3);
        assert!((lines[1][0].pos[1] + 1000.0).abs() < 1e-3 && (lines[1][1].pos[1] - 1000.0).abs() < 1e-3);
    }

    /* The grid pass keeps the visible line of vertices 1512/1513 (x = 1, y from -75 to 75), and
     * drops the discarded ones (vertex 0 is outside the clip rectangle, vertex 1508 is occluded). */
    #[test]
    fn grid_pass_lines_drop_discarded() {
        let (g, rv3d) = default_grid();
        let lines = grid_pass_lines(&g, g.grid_flag, 0, &rv3d);
        assert!(lines.len() < (4 * GRID_STEPS_DRAW * g.num_lines / 2) as usize);
        let found = lines.iter().any(|l| {
            (l[0].pos[0] - 1.0).abs() < 1e-5
                && (l[0].pos[1] + 75.0).abs() < 1e-4
                && (l[1].pos[0] - 1.0).abs() < 1e-5
                && (l[1].pos[1] - 75.0).abs() < 1e-4
        });
        assert!(found);
        assert!(lines.iter().all(|l| !((l[0].pos[0] - 0.0).abs() < 1e-6 && (l[0].pos[1] + 75.0).abs() < 1e-4)));
    }
}
