//! The playhead of the timeline's time scrub region.
//!
//! Ported from Blender:
//!  - `ED_time_scrub_draw_current_frame()`, `draw_current_frame()`
//!    (editors/interface/time_scrub_ui.cc:260-330)
//!  - `get_playhead_dimensions()`, `draw_playhead_stalk()`, `draw_playhead_box()`,
//!    `draw_playhead_tip()`                    (time_scrub_ui.cc:73-217)
//!  - `view2d_view_to_region_x()`              (editors/interface/view2d/view2d.cc:1673)
//!  - `TH_CFRAME` is `common.anim.playhead` (resources.cc:530), `TH_BACK` shaded by -20 gives the
//!    outline / shadow colour; the timeline's `back` has alpha 0, so that colour is fully
//!    transparent and only the fill of the stalk, box and tip is visible.
//!
//! The timeline view is the initial one of `action_main_region_init`-style setup (space_action.cc:133-153):
//! `cur = tot = [sfra - 10, efra + 10]` of the startup scene (frames 1 to 250), mask = whole region, and the
//! current frame is 1. The frame number (`ED_time_scrub_frame_str_get()`, `"%d"`) is drawn with the widget
//! font in `TH_TEXT_HI`, and the box is `max(text width + 8 * UI_SCALE_FAC, 24 * UI_SCALE_FAC)` wide.

use crate::area::{time_scrub_rect, Rcti};
use crate::font;
use crate::theme::{self, Rgba};
use crate::userdef::UserDef;
use crate::widget_draw::{blend_px, smoothstep};

/// `Scene::r.sfra`, `Scene::r.efra` and `Scene::r.cfra` of the startup scene.
pub const SFRA: f32 = 1.0;
pub const EFRA: f32 = 250.0;
/// `MINAFRAME` / `MAXFRAME` (DNA_scene_types.h): the range a frame can be set to.
const MINAFRAME: i32 = 0;
const MAXFRAME: i32 = 1_048_574;

/// `view2d_region_to_view_x()` (editors/interface/view2d/view2d.cc:1640) for the timeline's initial view
/// (`cur = [sfra - 10, efra + 10]`, mask = the window region, size `BLI_rcti_size_x()` without +1),
/// rounded to a whole frame: the frame under `region_x` (pixels from the left of the window region).
pub fn frame_from_region_x(window: Rcti, region_x: f32) -> i32 {
    let cur_xmin = SFRA - 10.0;
    let cur_size = (EFRA + 10.0) - cur_xmin;
    let frame = cur_xmin + cur_size * region_x / window.size_x() as f32;
    (frame.round() as i32).clamp(MINAFRAME, MAXFRAME)
}

/// `U.v2d_min_gridsize` default (DNA_userdef_types.h:1002); `MIN_MAJOR_LINE_DISTANCE` scales it by `UI_SCALE_FAC`.
const V2D_MIN_GRIDSIZE: f32 = 45.0;
/// `round_db_to_int(scene->frames_per_second())` of the startup scene: the `base` that
/// `action_main_region_draw()` passes to `ED_time_scrub_draw()` (space_action.cc:298-299).
const FPS: i32 = 24;

/// `get_divisor()` (editors/interface/view2d/view2d_draw.cc): a divisor of `distance` out of 2, 3 and 5,
/// preferring the one whose result is 2, else the first that divides cleanly, else `distance` itself.
fn get_divisor(distance: i32) -> i32 {
    let divisors = [2, 3, 5];
    let mut divides_no_remainder = [false; 3];
    for (i, divisor) in divisors.into_iter().enumerate() {
        let result = distance / divisor;
        let has_no_remainder = result * divisor == distance;
        if has_no_remainder && result == 2 {
            return divisor;
        }
        divides_no_remainder[i] = has_no_remainder;
    }
    for (i, divisor) in divisors.into_iter().enumerate() {
        if divides_no_remainder[i] {
            return divisor;
        }
    }
    distance
}

/// `calculate_grid_step()` (view2d_draw.cc): the distance in frames between major lines, at least 1.
fn grid_step(base: i32, pixel_width: f32, view_width: f32, min_distance: f32) -> i32 {
    if view_width.abs() < f32::EPSILON {
        return 1;
    }
    let pixels_per_view_unit = pixel_width / view_width;
    let mut distance = base.max(1);
    if pixels_per_view_unit * distance as f32 > min_distance {
        /* Shrink the distance. */
        while distance > 1 {
            let result = distance / get_divisor(distance);
            if pixels_per_view_unit * (result as f32) < min_distance {
                break;
            }
            distance = result;
        }
    } else {
        /* Grow the distance, doubling every time. */
        while pixels_per_view_unit * (distance as f32) < min_distance && distance < (1 << 30) {
            distance *= 2;
        }
    }
    distance
}

/// `ED_time_scrub_draw()` numbers (editors/animation/time_scrub_ui.cc:340-366) through `view2d_draw_scale_x()` and
/// `draw_horizontal_scale_indicators()` (view2d_draw.cc), for the timeline's window region `window` with
/// discrete frames (no fractions): a label every `calculate_grid_step()` frames, centred on its frame
/// (`x - trunc(width / 2)`) in `TH_TIME_SCRUB_TEXT`, on the baseline `numbers_rect.ymin + 4 * UI_SCALE_FAC`.
/// The minimum label distance is `max(MIN_MAJOR_LINE_DISTANCE, label_width)`, where `get_label_width()` measures
/// the view bounds formatted with `"%.3f"` (step 0) plus 6 px. Labels are not clipped to the region.
pub fn draw_scrub_numbers(buf: &mut [u32], width: usize, height: usize, user: &UserDef, window: Rcti) {
    let scale = user.scale_factor;
    let winx = (window.size_x() + 1) as f32;
    let mask_w = window.size_x() as f32;
    if mask_w <= 0.0 {
        return;
    }
    let cur_xmin = SFRA - 10.0;
    let cur_size = (EFRA + 10.0) - cur_xmin;

    let label = |v: f32| font::string_width(user, &format!("{:.3}", v.abs())) as f32;
    let label_width = label(cur_xmin).max(label(EFRA + 10.0)) + 6.0;
    let min_distance = (V2D_MIN_GRIDSIZE * scale).max(label_width);
    let step = grid_step(FPS, winx, cur_size, min_distance) as f32;

    /* `get_parallel_lines_draw_steps()` for the view bounds of the region rect `0 .. winx`
     * (`view2d_region_to_view_x()` divides by the mask size without the +1). */
    let bounds = [cur_xmin, cur_xmin + cur_size * winx / mask_w];
    let start = (bounds[0] / step).ceil() * step;
    if !(bounds[0] <= start && bounds[1] >= start) {
        return;
    }
    let steps = ((bounds[1] - start) / step).floor().max(0.0) as u32 + 1;
    if steps as f32 >= winx {
        return;
    }

    let scrub = time_scrub_rect(user, window);
    let winy = window.size_y() + 1;
    let cent_y = ((scrub.ymin - window.ymin) + winy) / 2;
    let centered_y = (cent_y as f32 - 4.0 * scale) as i32;
    let numbers_ymin = (centered_y as f32 - 4.0 * scale) as i32;
    let baseline = window.ymin + (numbers_ymin as f32 + 4.0 * scale).round() as i32;

    for i in 0..steps {
        let frame = start + i as f32 * step;
        let region_x = (frame - cur_xmin) / cur_size * winx;
        let text = (frame as i32).to_string();
        let text_width = font::string_width(user, &text) as f32;
        let x = window.xmin + (region_x - (text_width / 2.0).trunc()).round() as i32;
        font::draw_text(buf, width, height, user, x, baseline, &text, theme::SCRUBBING_TEXT);
    }
}


fn blend(buf: &mut [u32], width: usize, height: usize, x: i32, y: i32, color: Rgba, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let idx = (height - 1 - y as usize) * width + x as usize;
    buf[idx] = blend_px(buf[idx], color, alpha);
}

/// `draw_roundbox_4fv_ex()` for one inner colour: SDF of `widget_base` over the float `rect`
/// (xmin, xmax, ymin, ymax in window pixels, y up), outline band inside the rect.
#[allow(clippy::too_many_arguments)]
fn fill_round_box(
    buf: &mut [u32],
    width: usize,
    height: usize,
    clip: Rcti,
    rect: [f32; 4],
    inner: Rgba,
    outline: Rgba,
    line_w: f32,
    rad: f32,
) {
    let [xmin, xmax, ymin, ymax] = rect;
    let (size_x, size_y) = (xmax - xmin, ymax - ymin);
    if size_x <= 0.0 || size_y <= 0.0 {
        return;
    }
    let rad = rad.min(size_x.min(size_y) * 0.5);
    let x0 = (xmin.floor() as i32).max(clip.xmin);
    let x1 = ((xmax.ceil() as i32) - 1).min(clip.xmax);
    let y0 = (ymin.floor() as i32).max(clip.ymin);
    let y1 = ((ymax.ceil() as i32) - 1).min(clip.ymax);
    let (inner_a, outline_a) = (inner.a as f32 / 255.0, outline.a as f32 / 255.0);

    for y in y0..=y1 {
        let v = (y as f32 + 0.5) - ymin;
        for x in x0..=x1 {
            let u = (x as f32 + 0.5) - xmin;
            let ux = if u > size_x * 0.5 { size_x - u } else { u } - rad;
            let uy = if v > size_y * 0.5 { size_y - v } else { v } - rad;
            let inner_sdf = ux.min(uy).max(0.0);
            let outer_sdf = -(ux.min(0.0).powi(2) + uy.min(0.0).powi(2)).sqrt();
            let sdf = inner_sdf + outer_sdf + rad;

            let mask_border = smoothstep(-0.5, 0.5, sdf);
            let mask_inner = smoothstep(-0.5, 0.5, sdf - line_w);
            blend(buf, width, height, x, y, outline, outline_a * (mask_border - mask_inner).max(0.0));
            blend(buf, width, height, x, y, inner, inner_a * mask_inner);
        }
    }
}

/// Fills a `GPU_PRIM_TRI_STRIP` of 4 points (window pixels, y up) with polygon smoothing,
/// approximated by 4x4 supersampling per pixel.
fn fill_strip(buf: &mut [u32], width: usize, height: usize, clip: Rcti, pts: [[f32; 2]; 4], color: Rgba) {
    let alpha = color.a as f32 / 255.0;
    if alpha <= 0.0 {
        return;
    }
    let edge = |a: [f32; 2], b: [f32; 2], p: [f32; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    let inside = |t: [[f32; 2]; 3], p: [f32; 2]| {
        let (e0, e1, e2) = (edge(t[0], t[1], p), edge(t[1], t[2], p), edge(t[2], t[0], p));
        (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0)
    };
    let tris = [[pts[0], pts[1], pts[2]], [pts[1], pts[2], pts[3]]];

    let min_x = pts.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
    let max_x = pts.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
    let min_y = pts.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    let max_y = pts.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    let (x0, x1) = ((min_x.floor() as i32).max(clip.xmin), (max_x.ceil() as i32).min(clip.xmax));
    let (y0, y1) = ((min_y.floor() as i32).max(clip.ymin), (max_y.ceil() as i32).min(clip.ymax));

    for y in y0..=y1 {
        for x in x0..=x1 {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let p = [x as f32 + (sx as f32 + 0.5) / 4.0, y as f32 + (sy as f32 + 0.5) / 4.0];
                    if inside(tris[0], p) || inside(tris[1], p) {
                        hits += 1;
                    }
                }
            }
            blend(buf, width, height, x, y, color, alpha * hits as f32 / 16.0);
        }
    }
}

/// `ED_time_scrub_draw_current_frame()` for the timeline's window region `window` (inclusive Blender
/// `ANIM_draw_framerange()` (editors/animation/anim_draw.cc:152-186) for the timeline's window region
/// `window`, drawn in `action_main_region_draw()` (space_action.cc:249-252) only when the region is
/// higher than `UI_ANIM_MINY`: the area outside `[sfra, efra]` is darkened with `TH_BACK` shaded by -25
/// and alpha offset -100 (the timeline `back` has alpha 0, so this is fully transparent by default),
/// then 1 px lines at `sfra` and `efra` are drawn with `TH_BACK` shaded by -60.
pub fn draw_framerange(buf: &mut [u32], width: usize, height: usize, user: &UserDef, window: Rcti) {
    if width == 0 || height == 0 || buf.len() < width * height {
        return;
    }
    let buffer = Rcti { xmin: 0, xmax: width as i32 - 1, ymin: 0, ymax: height as i32 - 1 };
    let Some(clip) = window.isect(&buffer) else { return };

    /* `UI_ANIM_MINY` is `HEADERY * UI_SCALE_FAC * 1.1`; `area_headersize()` is `HEADERY` scaled. */
    let min_height = (crate::area::area_headersize(user) as f32 * 1.1) as i32;
    if window.size_y() < min_height {
        return;
    }

    let winx = (window.size_x() + 1) as f32;
    let cur_xmin = SFRA - 10.0;
    let cur_size = (EFRA + 10.0) - cur_xmin;
    let to_region_x = |frame: f32| (frame - cur_xmin) / cur_size * winx;

    let back = theme::SPACE_ACTION_BACK;
    let shade = |c: u8, offset: i32| (c as i32 + offset).clamp(0, 255) as u8;

    /* Darkened area outside the frame range (`immUniformThemeColorShadeAlpha(TH_BACK, -25, -100)`). */
    let dark = Rgba {
        r: shade(back.r, -25),
        g: shade(back.g, -25),
        b: shade(back.b, -25),
        a: (back.a as i32 - 100).clamp(0, 255) as u8,
    };
    if dark.a > 0 {
        let alpha = dark.a as f32 / 255.0;
        let (x_sfra, x_efra) = (to_region_x(SFRA), to_region_x(EFRA));
        for y in clip.ymin..=clip.ymax {
            for x in clip.xmin..=clip.xmax {
                let rx = (x - window.xmin) as f32 + 0.5;
                if rx < x_sfra || rx >= x_efra {
                    blend(buf, width, height, x, y, dark, alpha);
                }
            }
        }
    }

    /* Thin lines where the actual frames are (`immUniformThemeColorShade(TH_BACK, -60)`, opaque). */
    let line = Rgba { r: shade(back.r, -60), g: shade(back.g, -60), b: shade(back.b, -60), a: 255 };
    for frame in [SFRA, EFRA] {
        let x = window.xmin + to_region_x(frame).floor() as i32;
        if x < clip.xmin || x > clip.xmax {
            continue;
        }
        for y in clip.ymin..=clip.ymax {
            blend(buf, width, height, x, y, line, 1.0);
        }
    }
}
/// rect, y up) of the `0x00RRGGBB` buffer, with the stalk and the tip.
pub fn draw_playhead(buf: &mut [u32], width: usize, height: usize, user: &UserDef, window: Rcti, frame: i32) {
    if width == 0 || height == 0 || buf.len() < width * height {
        return;
    }
    let buffer = Rcti { xmin: 0, xmax: width as i32 - 1, ymin: 0, ymax: height as i32 - 1 };
    let Some(clip) = window.isect(&buffer) else { return };

    let scrub = time_scrub_rect(user, window);
    let winx = (window.size_x() + 1) as f32;
    let winy = (window.size_y() + 1) as f32;
    let scale = user.scale_factor;
    let pixelsize = user.pixelsize as f32;
    let (ox, oy) = (window.xmin as f32, window.ymin as f32);
    let scrub_ymin = (scrub.ymin - window.ymin) as f32;
    let scrub_ymax = winy;

    /* `view2d_view_to_region_x()` with `cur = [sfra - 10, efra + 10]` and the mask being the region. */
    let cur_xmin = SFRA - 10.0;
    let cur_size = (EFRA + 10.0) - cur_xmin;
    let region_x = (frame as f32 - cur_xmin) / cur_size * winx;

    /* `get_playhead_dimensions()`: the box is the text plus `text_padding` on both sides, at least `box_min_width`. */
    let frame_str = frame.to_string();
    let text_width = font::string_width(user, &frame_str) as f32;
    let outline_w = scale.round().max(1.0);
    let text_padding = 4.0 * scale;
    let box_min_width = 24.0 * scale;
    let box_width = (text_width + 2.0 * text_padding).max(box_min_width);
    let box_margin = 2.0 * scale;

    let (stalk_xmin, stalk_xmax) = if scale < 0.91 {
        let xmin = region_x.floor() - 1.0;
        (xmin, xmin + pixelsize + 2.0)
    } else {
        ((region_x - pixelsize).floor() - outline_w, (region_x + pixelsize + 1.0).floor() + outline_w)
    };
    let stalk_ymax = scrub_ymin;

    let box_xmin = region_x - box_width / 2.0;
    let box_xmax = region_x + box_width / 2.0 + 1.0;
    let box_ymin = (scrub_ymin + (box_margin - outline_w)).floor();
    let box_ymax = (scrub_ymax - box_margin + outline_w).ceil();

    let tip_top = box_ymin + outline_w;
    let tip_bottom = stalk_ymax - outline_w;
    let tip_half_width = 6.0 * scale;

    /* Colours: `TH_CFRAME`, and `TH_BACK` shaded by -20 (the shade keeps the alpha). */
    let fg = theme::ANIM_PLAYHEAD;
    let back = theme::SPACE_ACTION_BACK;
    let bg = Rgba { r: back.r.saturating_sub(20), g: back.g.saturating_sub(20), b: back.b.saturating_sub(20), a: back.a };

    let pt = |x: f32, y: f32| [x + ox, y + oy];

    /* `draw_playhead_stalk()`: shadow of the tip, then the stalk. */
    fill_strip(
        buf,
        width,
        height,
        clip,
        [
            pt(region_x - tip_half_width - outline_w, tip_top),
            pt(region_x + tip_half_width + outline_w, tip_top),
            pt(stalk_xmin, tip_bottom),
            pt(stalk_xmax, tip_bottom),
        ],
        bg,
    );
    fill_round_box(
        buf,
        width,
        height,
        clip,
        [stalk_xmin + ox, stalk_xmax + ox, oy, stalk_ymax + oy],
        fg,
        bg,
        outline_w,
        0.0,
    );

    /* `draw_playhead_box()`. */
    fill_round_box(
        buf,
        width,
        height,
        clip,
        [box_xmin + ox, box_xmax + ox, box_ymin + oy, box_ymax + oy],
        fg,
        bg,
        outline_w,
        4.0 * scale,
    );

    /* Frame number: `fontstyle_draw_simple()` at `region_x - text_width / 2` on the baseline
     * `BLI_rcti_cent_y(scrub_region_rect) - int(points * UI_SCALE_FAC * 0.38)`; the scrub rect is
     * `ymin = ymax - UI_TIME_SCRUB_MARGIN_Y` .. `ymax = winy` in region coordinates. */
    let cent_y = ((scrub.ymin - window.ymin) + winy as i32) / 2;
    let text_y = cent_y - (font::UI_DEFAULT_TEXT_POINTS * scale * 0.38) as i32;
    font::draw_text(
        buf,
        width,
        height,
        user,
        window.xmin + (region_x - text_width / 2.0).round() as i32,
        window.ymin + text_y,
        &frame_str,
        theme::SPACE_ACTION_TEXT_HI,
    );

    /* `draw_playhead_tip()`. */
    fill_strip(
        buf,
        width,
        height,
        clip,
        [
            pt(region_x - tip_half_width, tip_top),
            pt(region_x + tip_half_width, tip_top),
            pt(stalk_xmin + outline_w, tip_bottom),
            pt(stalk_xmax - outline_w, tip_bottom),
        ],
        fg,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /* 300x100 timeline window region at 1x: the scrub strip is y 77..=99, frame 1 is at
     * x = 10 / 269 * 300 = 11.15, the stalk is x 9..=13 with a transparent 1 px outline, so x 11 is the
     * playhead colour below the strip and in the box; the far right stays untouched. */
    #[test]
    fn playhead_stalk_and_box() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (300usize, 100usize);
        let bg = theme::SCRUBBING_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let window = Rcti { xmin: 0, xmax: 299, ymin: 0, ymax: 99 };
        draw_playhead(&mut buf, w, h, &user, window, 1);

        let at = |x: usize, y: usize| buf[(h - 1 - y) * w + x];
        assert_eq!(at(11, 10), theme::ANIM_PLAYHEAD.to_xrgb());
        assert_eq!(at(11, 80), theme::ANIM_PLAYHEAD.to_xrgb());
        assert_eq!(at(200, 10), bg);
        assert_eq!(at(200, 90), bg);
    }

    /* With the font file present the frame number is drawn in `TH_TEXT_HI` inside the box, so some pixel of
     * the box is much lighter than the playhead colour; a three digit frame widens the box past 24 px. */
    #[test]
    fn frame_number_text_is_drawn() {
        let user = UserDef::from_window_scale(1.0);
        if font::string_width(&user, "1") == 0 {
            return;
        }
        let (w, h) = (300usize, 100usize);
        let bg = theme::SCRUBBING_BACK.to_xrgb();
        let window = Rcti { xmin: 0, xmax: 299, ymin: 0, ymax: 99 };
        let lighter = |buf: &[u32]| {
            (0..w).any(|x| (78..=99).any(|y| ((buf[(h - 1 - y) * w + x] >> 16) & 0xff) > 0x90))
        };

        let mut buf = vec![bg; w * h];
        draw_playhead(&mut buf, w, h, &user, window, 1);
        assert!(lighter(&buf));

        assert!(font::string_width(&user, "250") + 8 > 24);
    }

    /* `calculate_grid_step()` with base 24 over the view width 269 (frames -9 .. 260): 1570 px with at least
     * 50 px per label gives 12 (24 -> 12 -> stop, 6 would be 35 px); 300 px has to grow 24 to 48; 2690 px shrinks
     * 24 -> 12 -> 6 and stops before 2 (20 px). */
    #[test]
    fn grid_step_follows_the_zoom() {
        assert_eq!(grid_step(24, 1570.0, 269.0, 50.0), 12);
        assert_eq!(grid_step(24, 300.0, 269.0, 45.0), 48);
        assert_eq!(grid_step(24, 2690.0, 269.0, 45.0), 6);
        assert_eq!(grid_step(24, 100.0, 0.0, 45.0), 1);
        assert_eq!(get_divisor(24), 2);
        assert_eq!(get_divisor(6), 3);
        assert_eq!(get_divisor(7), 7);
    }

    /* With the font file present, a 300 px wide timeline (step 48) has a label for frame 48 at
     * x = 57 / 269 * 300 = 63.6 in the scrub strip (y 77..=99); the area below the strip keeps the background. */
    #[test]
    fn scrub_numbers_are_drawn() {
        let user = UserDef::from_window_scale(1.0);
        if font::string_width(&user, "48") == 0 {
            return;
        }
        let (w, h) = (300usize, 100usize);
        let bg = theme::SCRUBBING_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let window = Rcti { xmin: 0, xmax: 299, ymin: 0, ymax: 99 };
        draw_scrub_numbers(&mut buf, w, h, &user, window);

        let at = |x: usize, y: usize| buf[(h - 1 - y) * w + x];
        assert!((55..=72).any(|x| (77..=99).any(|y| at(x, y) != bg)));
        assert!((0..w).all(|x| (0..77).all(|y| at(x, y) == bg)));
    }
}
