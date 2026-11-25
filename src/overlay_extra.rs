//! The overlay shapes of the camera, light and empty objects.
//!
//! Modelled on Blender:
//!  - the camera (`overlay_camera.hh`, `BKE_camera_view_frame_ex()`, blenkernel/intern/camera.cc:738): the
//!    frustum is a pyramid from the camera origin to a frame at depth `drawsize / 2 * lens / (sensor_x / 2)`
//!    along -Z (display size 1 m, 50 mm lens, 36 mm sensor: depth 1.3889 m), the frame is `drawsize / 2` wide
//!    and, for the default 1920 x 1080 render size, 0.5625 times as high (`sensor_fit` auto); a triangle sits
//!    above the frame. Deviation: the triangle's exact size comes from the camera triangle shape, whose source
//!    was not read; it is drawn as 0.7 * `drawsize / 2` high on a base of the same width.
//!  - the light (`overlay_light.hh`): a circle that always faces the viewer at a fixed size in pixels, and the
//!    line down to the ground plane (`z = 0`) for a light that is not on it. The other light shapes (sun
//!    direction line, spot cone, area rectangle) are not modelled.
//!
//!  - the empty (`overlay_empty.hh`, `OB_PLAINAXES`): three lines through the origin along the local axes, one
//!    unit long on each side (display size 1 m). The other empty shapes (arrows, circle, cube, sphere, cone,
//!    image) are not modelled.
//!
//! Colours are those of the wire of the object: `TH_WIRE` unselected, `TH_SELECT` selected, `TH_ACTIVE` active.
//! The lines are not depth tested. Hidden objects are not drawn.

use crate::area::Rcti;
use crate::mesh::V3;
use crate::overlay_cursor::{draw_segment_gradient, project_segment};
use crate::overlay_grid::srgb_to_linearrgb;
use crate::scene::{mat4_mul_dir, mat4_mul_point, Object, ObjectData, Scene};
use crate::theme::{Rgba, SPACE_VIEW3D_ACTIVE, SPACE_VIEW3D_SELECT, SPACE_VIEW3D_WIRE};
use crate::userdef::UserDef;
use crate::view3d::RegionView3D;

/// `Camera::drawsize` (display size) of the default camera.
const DRAW_SIZE: f32 = 1.0;
const LENS: f32 = 50.0;
const SENSOR_X: f32 = 36.0;
/// Height over width of the default render size (1080 / 1920) with `sensor_fit` auto.
const ASPECT_Y: f32 = 1080.0 / 1920.0;
/// The radius of the light circle in pixels (before `U.pixelsize`).
const LIGHT_RADIUS_PX: f32 = 11.0;
const LIGHT_SEGMENTS: usize = 24;
/// The distance in pixels within which a click picks a camera or light.
const PICK_RADIUS_PX: f32 = 6.0;

type Seg = (V3, V3);

fn lin(c: Rgba) -> [f32; 3] {
    [srgb_to_linearrgb(c.r as f32 / 255.0), srgb_to_linearrgb(c.g as f32 / 255.0), srgb_to_linearrgb(c.b as f32 / 255.0)]
}

/// The camera shape in object space: 4 pyramid edges, the 4 frame edges and the 3 edges of the triangle.
pub fn camera_segments() -> Vec<Seg> {
    let size = DRAW_SIZE / 2.0;
    let depth = -size * LENS / (SENSOR_X / 2.0);
    let (fx, fy) = (size, size * ASPECT_Y);
    let corners: [V3; 4] = [[-fx, -fy, depth], [fx, -fy, depth], [fx, fy, depth], [-fx, fy, depth]];
    let mut segs = Vec::new();
    for i in 0..4 {
        segs.push(([0.0; 3], corners[i]));
        segs.push((corners[i], corners[(i + 1) % 4]));
    }
    let (margin, height) = (0.1 * size, 0.7 * size);
    let base = fy + margin;
    let tri: [V3; 3] = [[-height / 2.0, base, depth], [height / 2.0, base, depth], [0.0, base + height, depth]];
    for i in 0..3 {
        segs.push((tri[i], tri[(i + 1) % 3]));
    }
    segs
}

/// The world space edges of one object's shape; `px` is the world size of one pixel at the object (the light).
fn world_segments(o: &Object, rv3d: &RegionView3D, winx: i32, winy: i32) -> Vec<Seg> {
    let m = o.world_matrix();
    match o.data {
        ObjectData::Camera => camera_segments().into_iter().map(|(a, b)| (mat4_mul_point(&m, a), mat4_mul_point(&m, b))).collect(),
        ObjectData::Light => {
            let zfac = rv3d.calc_zfac(o.loc);
            let d = rv3d.win_to_delta(winx, winy, zfac, LIGHT_RADIUS_PX, 0.0);
            let radius = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            /* The circle faces the viewer: it spans the view's X and Y axes. */
            let (ax, ay) = (mat4_mul_dir(&rv3d.viewinv, [1.0, 0.0, 0.0]), mat4_mul_dir(&rv3d.viewinv, [0.0, 1.0, 0.0]));
            let point = |k: usize| -> V3 {
                let a = std::f32::consts::TAU * k as f32 / LIGHT_SEGMENTS as f32;
                std::array::from_fn(|i| o.loc[i] + radius * (a.cos() * ax[i] + a.sin() * ay[i]))
            };
            let mut segs: Vec<Seg> = (0..LIGHT_SEGMENTS).map(|k| (point(k), point(k + 1))).collect();
            if o.loc[2].abs() > 1e-4 {
                segs.push((o.loc, [o.loc[0], o.loc[1], 0.0]));
            }
            segs
        }
        ObjectData::Empty => plain_axes().into_iter().map(|(a, b)| (mat4_mul_point(&m, a), mat4_mul_point(&m, b))).collect(),
        ObjectData::Mesh(_) => Vec::new(),
    }
}

/// The empty's Plain Axes in object space.
pub fn plain_axes() -> Vec<Seg> {
    (0..3)
        .map(|k| {
            let (mut a, mut b) = ([0.0; 3], [0.0; 3]);
            a[k] = -1.0;
            b[k] = 1.0;
            (a, b)
        })
        .collect()
}

fn is_extra(o: &Object) -> bool {
    matches!(o.data, ObjectData::Camera | ObjectData::Light | ObjectData::Empty) && !o.hidden
}

/// Draws the cameras and lights of `scene` into `region` (inclusive Blender rect, y up) of the `0x00RRGGBB`
/// buffer. `rv3d` must have been updated for the size of the region.
pub fn draw_extras(buf: &mut [u32], width: usize, height: usize, region: Rcti, user: &UserDef, rv3d: &RegionView3D, scene: &Scene) {
    let size = (region.size_x() + 1, region.size_y() + 1);
    if size.0 <= 0 || size.1 <= 0 {
        return;
    }
    let line_width = user.pixelsize as f32;
    /* Unselected first, then the selected, the active one last (on top). */
    let rank = |o: &Object| if !o.selected { 0 } else if scene.active == Some(o.id) { 2 } else { 1 };
    for r in 0..3 {
        for o in scene.objects.iter().filter(|o| is_extra(o) && rank(o) == r) {
            let color = lin([SPACE_VIEW3D_WIRE, SPACE_VIEW3D_SELECT, SPACE_VIEW3D_ACTIVE][r]);
            for (a, b) in world_segments(o, rv3d, size.0, size.1) {
                if let Some((s0, s1)) = project_segment(rv3d, a, b, size) {
                    draw_segment_gradient(buf, width, height, region, size, s0, s1, color, color, line_width);
                }
            }
        }
    }
}

fn dist_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 1e-9 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (qx, qy) = (a[0] + t * dx, a[1] + t * dy);
    ((p[0] - qx).powi(2) + (p[1] - qy).powi(2)).sqrt()
}

/// The camera or light whose drawn shape passes within a few pixels of the region pixel `mval` (y up), the
/// nearest one. Used when a click hits no mesh.
pub fn pick(scene: &Scene, rv3d: &RegionView3D, winx: i32, winy: i32, mval: [i32; 2]) -> Option<u32> {
    let p = [mval[0] as f32 + 0.5, mval[1] as f32 + 0.5];
    let mut best: Option<(f32, u32)> = None;
    for o in scene.objects.iter().filter(|o| is_extra(o)) {
        for (a, b) in world_segments(o, rv3d, winx, winy) {
            let Some((s0, s1)) = project_segment(rv3d, a, b, (winx, winy)) else { continue };
            let d = dist_to_segment(p, s0, s1);
            if d <= PICK_RADIUS_PX && best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, o.id));
            }
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view3d::View3D;

    const W: i32 = 200;
    const H: i32 = 100;

    fn view() -> RegionView3D {
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), W, H);
        rv3d
    }

    /* A scene with the camera and the light at the origin and nothing else, so the origin is the view centre. */
    fn scene_with(extra: &str) -> Scene {
        let mut s = Scene::startup();
        s.select_all(crate::scene::SelectAction::Deselect);
        for o in &mut s.objects {
            o.hidden = o.name != extra;
            o.loc = [0.0; 3];
            o.rot = [0.0; 3];
        }
        s
    }

    #[test]
    fn the_camera_shape_has_eleven_edges_at_the_default_depth() {
        let segs = camera_segments();
        assert_eq!(segs.len(), 11);
        let (_, corner) = segs[0];
        assert!((corner[2] + 1.3889).abs() < 1e-3 && (corner[0].abs() - 0.5).abs() < 1e-6, "{corner:?}");
        assert!((corner[1].abs() - 0.28125).abs() < 1e-6);
        /* The pyramid edges start at the origin. */
        assert_eq!(segs.iter().filter(|(a, _)| *a == [0.0; 3]).count(), 4);
    }

    #[test]
    fn the_camera_follows_its_transform() {
        let mut s = Scene::startup();
        let cam = s.objects.iter_mut().find(|o| o.name == "Camera").unwrap();
        cam.loc = [1.0, 2.0, 3.0];
        cam.rot = [0.0; 3];
        cam.scale = [2.0, 2.0, 2.0];
        let o = s.objects.iter().find(|o| o.name == "Camera").unwrap().clone();
        let segs = world_segments(&o, &view(), W, H);
        assert_eq!(segs[0].0, [1.0, 2.0, 3.0]);
        assert!((segs[0].1[2] - (3.0 - 2.0 * 1.3889)).abs() < 1e-3);
    }

    #[test]
    fn the_light_has_a_ground_line_only_above_the_ground() {
        let mut s = Scene::startup();
        let light = s.objects.iter_mut().find(|o| o.name == "Light").unwrap();
        light.loc = [1.0, 1.0, 4.0];
        let o = s.objects.iter().find(|o| o.name == "Light").unwrap().clone();
        let segs = world_segments(&o, &view(), W, H);
        assert_eq!(segs.len(), LIGHT_SEGMENTS + 1);
        assert_eq!(segs.last().unwrap().1, [1.0, 1.0, 0.0]);
        let mut grounded = o.clone();
        grounded.loc[2] = 0.0;
        assert_eq!(world_segments(&grounded, &view(), W, H).len(), LIGHT_SEGMENTS);
    }

    #[test]
    fn the_light_circle_has_a_constant_size_in_pixels() {
        let rv3d = view();
        let s = Scene::startup();
        let id = s.objects.iter().find(|o| o.name == "Light").unwrap().id;
        let radius_px = |loc: V3| {
            let mut o = s.objects.iter().find(|o| o.id == id).unwrap().clone();
            o.loc = loc;
            let (a, _) = world_segments(&o, &rv3d, W, H)[0];
            let centre = project_segment(&rv3d, o.loc, [o.loc[0], o.loc[1], o.loc[2] + 1e-3], (W, H)).unwrap().0;
            let edge = project_segment(&rv3d, a, [a[0], a[1], a[2] + 1e-3], (W, H)).unwrap().0;
            ((edge[0] - centre[0]).powi(2) + (edge[1] - centre[1]).powi(2)).sqrt()
        };
        let (near, far) = (radius_px([0.0, 0.0, 0.0]), radius_px([0.0, 3.0, 0.0]));
        assert!((near - LIGHT_RADIUS_PX).abs() < 1.0 && (far - LIGHT_RADIUS_PX).abs() < 1.0, "{near} {far}");
    }

    #[test]
    fn extras_are_drawn_and_hidden_ones_are_not() {
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: W - 1, ymin: 0, ymax: H - 1 };
        let bg = crate::theme::SPACE_VIEW3D_BACK.to_xrgb();
        for name in ["Camera", "Light"] {
            let mut s = scene_with(name);
            let mut buf = vec![bg; (W * H) as usize];
            draw_extras(&mut buf, W as usize, H as usize, region, &user, &view(), &s);
            assert!(buf.iter().any(|&p| p != bg), "{name} is drawn");
            for o in &mut s.objects {
                o.hidden = true;
            }
            let mut buf = vec![bg; (W * H) as usize];
            draw_extras(&mut buf, W as usize, H as usize, region, &user, &view(), &s);
            assert!(buf.iter().all(|&p| p == bg), "{name} hidden");
        }
    }

    #[test]
    fn a_selected_camera_is_drawn_in_the_select_colour() {
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: W - 1, ymin: 0, ymax: H - 1 };
        let bg = crate::theme::SPACE_VIEW3D_BACK.to_xrgb();
        let mut s = scene_with("Camera");
        let id = s.objects.iter().find(|o| o.name == "Camera").unwrap().id;
        s.select_pick(Some(id), false);
        s.active = None;
        let mut buf = vec![bg; (W * H) as usize];
        draw_extras(&mut buf, W as usize, H as usize, region, &user, &view(), &s);
        assert!(buf.iter().any(|&p| ((p >> 16) & 0xff) > (p & 0xff) + 100), "orange pixels");
    }

    #[test]
    fn clicking_a_camera_line_picks_it() {
        let s = scene_with("Camera");
        let id = s.objects.iter().find(|o| o.name == "Camera").unwrap().id;
        let rv3d = view();
        /* The pyramid apex is the view centre. */
        assert_eq!(pick(&s, &rv3d, W, H, [100, 50]), Some(id));
        assert_eq!(pick(&s, &rv3d, W, H, [2, 2]), None);
    }

    #[test]
    fn hidden_extras_are_not_picked() {
        let mut s = scene_with("Light");
        s.objects.iter_mut().for_each(|o| o.hidden = true);
        assert_eq!(pick(&s, &view(), W, H, [100, 50]), None);
    }

    #[test]
    fn segment_distance() {
        assert_eq!(dist_to_segment([5.0, 3.0], [0.0, 0.0], [10.0, 0.0]), 3.0);
        assert_eq!(dist_to_segment([-4.0, 3.0], [0.0, 0.0], [10.0, 0.0]), 5.0);
        assert_eq!(dist_to_segment([1.0, 1.0], [2.0, 2.0], [2.0, 2.0]), 2.0f32.sqrt());
    }

    #[test]
    fn the_empty_is_three_axes_through_the_origin_and_can_be_picked_and_drawn() {
        let segs = plain_axes();
        assert_eq!(segs.len(), 3);
        assert!(segs.iter().all(|(a, b)| (0..3).filter(|&k| a[k] != 0.0 || b[k] != 0.0).count() == 1));

        let mut s = Scene::startup();
        for o in &mut s.objects {
            o.hidden = true;
        }
        let id = s.add_item(crate::add_menu::AddItem::Empty, [0.0; 3]);
        let rv3d = view();
        assert_eq!(pick(&s, &rv3d, W, H, [W / 2, H / 2]), Some(id), "the origin is the view centre");
        assert_eq!(pick(&s, &rv3d, W, H, [2, 2]), None);

        let mut buf = vec![0u32; (W * H) as usize];
        let region = Rcti { xmin: 0, xmax: W - 1, ymin: 0, ymax: H - 1 };
        let user = UserDef::from_window_scale(1.0);
        draw_extras(&mut buf, W as usize, H as usize, region, &user, &rv3d, &s);
        assert!(buf.iter().any(|&p| p != 0), "something was drawn");
        s.objects.iter_mut().for_each(|o| o.hidden = true);
        let mut blank = vec![0u32; (W * H) as usize];
        draw_extras(&mut blank, W as usize, H as usize, region, &user, &rv3d, &s);
        assert!(blank.iter().all(|&p| p == 0), "hidden objects are not drawn");
    }
}
