//! The navigate buttons of the 3D viewport (zoom, pan, camera, perspective/orthographic), `VIEW3D_GGT_navigate`
//! (view3d_gizmo_navigate.cc:285-380). Deviation: only drawn, the buttons do not act yet; the backdrop colour is
//! an approximation of the button gizmo's backdrop.

use crate::area::Rcti;
use crate::icons::{draw_icon, Icon};
use crate::theme::Rgba;
use crate::userdef::UserDef;
use crate::widget_draw::blend_px;

/// `GIZMO_MINI_SIZE`, `GIZMO_MINI_OFFSET`, `GIZMO_SIZE` (`U.gizmo_size_navigate_v3d`) and `GIZMO_OFFSET`.
const GIZMO_MINI_SIZE: f32 = 28.0;
const GIZMO_MINI_OFFSET: f32 = 2.0;
const GIZMO_SIZE: f32 = 80.0;
const GIZMO_OFFSET: f32 = 10.0;
const BACKDROP: Rgba = Rgba::hex(0x00000020);

/// The buttons below the axis gizmo (`USER_MINI_AXIS_TYPE_GIZMO`, `USER_SHOW_GIZMO_NAVIGATE`, no view lock) as
/// `GIZMO_MINI_SIZE` squares in window pixels (y up), top to bottom: zoom, pan, the camera toggle and, outside
/// the camera view, the perspective/orthographic toggle.
pub fn buttons(user: &UserDef, region: Rcti, ortho: bool, camera_view: bool) -> Vec<(Rcti, Icon)> {
    let s = user.scale_factor;
    let icon_offset = s * (GIZMO_OFFSET + GIZMO_SIZE / 2.0);
    let step = (GIZMO_MINI_SIZE + GIZMO_MINI_OFFSET) * s;
    let cx = (region.xmax as f32 - step * 0.75).round();
    let cy = (region.ymax as f32 - icon_offset * 2.2).round();
    let mut icons = vec![Icon::ViewZoom, Icon::ViewPan, if camera_view { Icon::ViewCamera } else { Icon::ViewCameraUnselected }];
    if !camera_view {
        icons.push(if ortho { Icon::ViewOrtho } else { Icon::ViewPerspective });
    }
    let half = (GIZMO_MINI_SIZE * s / 2.0).round() as i32;
    icons
        .into_iter()
        .enumerate()
        .map(|(slot, icon)| {
            let (x, y) = (cx as i32, (cy - (step * slot as f32).round()) as i32);
            (Rcti { xmin: x - half, xmax: x + half - 1, ymin: y - half, ymax: y + half - 1 }, icon)
        })
        .collect()
}

/// Draws the buttons: one rounded backdrop behind all of them (a capsule as wide as a button) and the icons.
pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti, ortho: bool, camera_view: bool) {
    let buttons = buttons(user, region, ortho, camera_view);
    let Some(first) = buttons.first().map(|b| b.0) else { return };
    let last = buttons.last().map(|b| b.0).unwrap_or(first);
    let pill = Rcti { xmin: first.xmin, xmax: first.xmax, ymin: last.ymin, ymax: first.ymax };
    let r = (pill.xmax - pill.xmin + 1) as f32 / 2.0;
    let cx = (pill.xmin + pill.xmax + 1) as f32 / 2.0;
    let (y_lo, y_hi) = (pill.ymin as f32 + r, pill.ymax as f32 + 1.0 - r);
    for y in pill.ymin..=pill.ymax {
        for x in pill.xmin..=pill.xmax {
            if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                continue;
            }
            let (px, py) = (x as f32 + 0.5, (y as f32 + 0.5).clamp(y_lo, y_hi));
            let d = ((px - cx).powi(2) + (y as f32 + 0.5 - py).powi(2)).sqrt();
            let cov = (r - d + 0.5).clamp(0.0, 1.0);
            if cov > 0.0 {
                let i = (height - 1 - y as usize) * width + x as usize;
                buf[i] = blend_px(buf[i], BACKDROP, cov * BACKDROP.a as f32 / 255.0);
            }
        }
    }
    for (rect, icon) in buttons {
        draw_icon(buf, width, height, user, rect, icon, Rgba::hex(0xffffffff));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_stack_below_the_axis_gizmo_in_blenders_order() {
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: 999, ymin: 0, ymax: 799 };
        let b = buttons(&user, region, false, false);
        let icons: Vec<Icon> = b.iter().map(|x| x.1).collect();
        assert_eq!(icons, [Icon::ViewZoom, Icon::ViewPan, Icon::ViewCameraUnselected, Icon::ViewPerspective]);
        /* centre x = 999 - 22.5 -> 977, first centre y = 799 - 110, then 30 px apart. */
        assert_eq!((b[0].0.xmin, b[0].0.xmax), (977 - 14, 977 + 13));
        assert_eq!(b[0].0.ymax, 799 - 110 + 13);
        assert_eq!(b[0].0.ymax - b[1].0.ymax, 30);
        assert_eq!(b[1].0.ymax - b[3].0.ymax, 60);
    }

    #[test]
    fn camera_view_drops_the_projection_toggle() {
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: 999, ymin: 0, ymax: 799 };
        let b = buttons(&user, region, true, true);
        assert_eq!(b.len(), 3);
        assert_eq!(b[2].1, Icon::ViewCamera);
        assert_eq!(buttons(&user, region, true, false)[3].1, Icon::ViewOrtho);
    }

    #[test]
    fn drawing_paints_inside_the_button_only() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (400usize, 400usize);
        let region = Rcti { xmin: 0, xmax: 399, ymin: 0, ymax: 399 };
        let mut buf = vec![0x0030_3030u32; w * h];
        draw(&mut buf, w, h, &user, region, false, false);
        let r = buttons(&user, region, false, false)[0].0;
        let at = |x: i32, y: i32| buf[(h - 1 - y as usize) * w + x as usize];
        assert_ne!(at((r.xmin + r.xmax) / 2, (r.ymin + r.ymax) / 2), 0x0030_3030);
        assert_eq!(at(r.xmin - 3, r.ymin + 14), 0x0030_3030);
    }
}
