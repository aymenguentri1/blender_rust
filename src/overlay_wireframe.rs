//! Wireframe shading (`OB_WIRE`) of the default cube.
//!
//! Ported from Blender:
//!  - `VIEW3D_OT_toggle_shading()`          (editors/space_view3d/view3d_edit.cc:1318-1355): the
//!    WIREFRAME type switches between wire and solid shading
//!  - `TH_WIRE`                             (userdef_default_theme.c:374): black wire colour of an
//!    unselected object
//!
//! The overlay wireframe shader source was not available, so the edges are drawn with the same
//! smooth polyline rasteriser as the 3D cursor: line width `U.pixelsize`, opaque, no depth test
//! (all 12 edges are visible, like the default X-ray wireframe). The cube is drawn unselected;
//! selection colours, wire opacity, `xray_alpha_wire` and the wireframe threshold are not modelled.

use crate::area::Rcti;
use crate::overlay_cursor::{draw_segment_gradient, project_segment};
use crate::overlay_grid::srgb_to_linearrgb;
use crate::theme::{SPACE_VIEW3D_BACK, SPACE_VIEW3D_WIRE};
use crate::userdef::UserDef;
use crate::view3d::RegionView3D;

/// Clips the world space segment `p0` -> `p1` against the view clipping planes (`RV3D_CLIPPING`), keeping
/// the part where `dot(plane, (co, 1)) >= 0`. `None` when the whole segment is clipped.
fn clip_segment(planes: Option<&[[f32; 4]; 4]>, p0: [f32; 3], p1: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
    let (mut p0, mut p1) = (p0, p1);
    if let Some(planes) = planes {
        for pl in planes {
            let dist = |v: [f32; 3]| pl[0] * v[0] + pl[1] * v[1] + pl[2] * v[2] + pl[3];
            let (d0, d1) = (dist(p0), dist(p1));
            if d0 < 0.0 && d1 < 0.0 {
                return None;
            }
            let t = d0 / (d0 - d1);
            let at = |t: f32| -> [f32; 3] { std::array::from_fn(|k| p0[k] + (p1[k] - p0[k]) * t) };
            if d0 < 0.0 {
                p0 = at(t);
            } else if d1 < 0.0 {
                p1 = at(t);
            }
        }
    }
    Some((p0, p1))
}

/// `abs(dot(ws_nor, V))` saturated, with `V = view.world_incident_vector(ws_pos)` (overlay_wireframe.bsl.hh
/// `vert_main()`): towards the camera for perspective, `viewinv[2]` for orthographic.
fn facing(rv3d: &RegionView3D, normal: [f32; 3], pos: [f32; 3]) -> f32 {
    let mut v = if rv3d.ortho {
        [rv3d.viewinv[2][0], rv3d.viewinv[2][1], rv3d.viewinv[2][2]]
    } else {
        [rv3d.viewinv[3][0] - pos[0], rv3d.viewinv[3][1] - pos[1], rv3d.viewinv[3][2] - pos[2]]
    };
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 0.0 {
        v = [v[0] / len, v[1] / len, v[2] / len];
    }
    (normal[0] * v[0] + normal[1] * v[1] + normal[2] * v[2]).abs().clamp(0.0, 1.0)
}

/// The vertex colour of `vert_main()`: rim and wire are mixed in a gamma 2.2 space,
/// `mix(rim, mix(rim, wire, 0.35), facing)`; `wire_opacity` is 1 (default `wireframe_opacity`).
fn wire_color(rim: [f32; 3], wire: [f32; 3], facing: f32) -> [f32; 3] {
    std::array::from_fn(|k| {
        let (r, w) = (rim[k].powf(1.0 / 2.2), wire[k].powf(1.0 / 2.2));
        let front = r + (w - r) * 0.35;
        (r + (front - r) * facing).powf(2.2)
    })
}


/// Draws the 12 edges of the 2 m cube at the origin into `region` (inclusive Blender rect, y up) of
/// the `0x00RRGGBB` buffer. `rv3d` must have been updated for the size of the region.
pub fn draw_cube_wire(
    buf: &mut [u32],
    width: usize,
    height: usize,
    region: Rcti,
    user: &UserDef,
    rv3d: &RegionView3D,
) {
    let size = (region.size_x() + 1, region.size_y() + 1);
    if size.0 <= 0 || size.1 <= 0 {
        return;
    }
    /* `wire_color_get()` of an unselected object: rim is `TH_WIRE`, wire is `TH_BACK` (linear). */
    let lin = |c: crate::theme::Rgba| {
        [srgb_to_linearrgb(c.r as f32 / 255.0), srgb_to_linearrgb(c.g as f32 / 255.0), srgb_to_linearrgb(c.b as f32 / 255.0)]
    };
    let (rim, wire) = (lin(SPACE_VIEW3D_WIRE), lin(SPACE_VIEW3D_BACK));
    let line_width = user.pixelsize as f32;

    for axis in 0..3 {
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        for sa in [-1.0f32, 1.0] {
            for sb in [-1.0f32, 1.0] {
                let mut p0 = [0.0f32; 3];
                p0[axis] = -1.0;
                p0[a] = sa;
                p0[b] = sb;
                let mut p1 = p0;
                p1[axis] = 1.0;
                let Some((p0, p1)) = clip_segment(rv3d.clip.as_ref(), p0, p1) else { continue };
                if let Some((s0, s1)) = project_segment(rv3d, p0, p1, size) {
                    /* The mesh extractor source for the wire `nor` attribute was not supplied: the edge is
                     * assumed to take the flat normal of the first adjacent face in `workbench::FACES` order. */
                    let mut normal = [0.0f32; 3];
                    let (ia, ib) = (a * 2 + (sa < 0.0) as usize, b * 2 + (sb < 0.0) as usize);
                    if ia < ib {
                        normal[a] = sa;
                    } else {
                        normal[b] = sb;
                    }
                    let c0 = wire_color(rim, wire, facing(rv3d, normal, p0));
                    let c1 = wire_color(rim, wire, facing(rv3d, normal, p1));
                    draw_segment_gradient(buf, width, height, region, size, s0, s1, c0, c1, line_width);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view3d::View3D;

    /* Default view, 200x100 region: the near face edge x = 1, z = 1 projects to x = 100 + 100 * 1.3889 / 9
     * = 115.4, so the pixel at (115, 50) is darkened towards black; the corner keeps the background. */
    #[test]
    fn cube_edges_are_drawn() {
        let (w, h) = (200usize, 100usize);
        let bg = crate::theme::SPACE_VIEW3D_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w as i32, h as i32);
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };
        let user = UserDef::from_window_scale(1.0);
        draw_cube_wire(&mut buf, w, h, region, &user, &rv3d);

        let at = |x: usize, y: usize| buf[(h - 1 - y) * w + x];
        assert!((at(115, 50) >> 16) & 0xff < 0x3d, "{:#08x}", at(115, 50));
        assert_eq!(at(0, 0), bg);
    }
}
