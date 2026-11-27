//! The icons of a collapsed object drawn after its name, and the circle behind active icons.
//!
//! Ported from Blender (editors/space_outliner):
//!  - `outliner_draw_iconrow()` / `outliner_draw_iconrow_doit()` (outliner_draw.cc:2875-3095): the direct
//!    children of the collapsed element; non-ID entries (the modifier base) are drawn as they come, ID entries
//!    (the object data) are merged by type and drawn last
//!  - `outliner_draw_tree_element()` (outliner_draw.cc:3355-3375): the row starts at the name's x plus
//!    `UI_UNIT_X` plus the name width
//!  - `outliner_icon_background_colors()` / `outliner_draw_active_indicator()` (outliner_draw.cc:2848-2872)

use crate::area::Rcti;
use crate::font;
use crate::icon_theme;
use crate::icons::{draw_icon, Icon};
use crate::scene::{LightKind, Object, ObjectData, RowKind, Scene};
use crate::theme::{self, Rgba};
use crate::userdef::UserDef;
use crate::widget_draw::blend_px;

/// `OL_Y_OFFSET`.
const OL_Y_OFFSET: i32 = 2;

/// The icons after the name of the (collapsed) object row, left to right.
pub fn child_icons(object: &Object) -> Vec<Icon> {
    let mut out = Vec::new();
    if object.has_eval() {
        out.push(Icon::ModWrench);
    }
    match object.data {
        ObjectData::Mesh(_) => out.push(Icon::MeshData),
        ObjectData::Camera => out.push(Icon::CameraData),
        ObjectData::Light(kind) => out.push(light_icon(kind)),
        ObjectData::Empty(_) => {}
    }
    out
}

/// `ID_LA` of `tree_element_get_icon_from_id()` (outliner_draw.cc:2599-2611).
pub fn light_icon(kind: LightKind) -> Icon {
    match kind {
        LightKind::Point => Icon::LightPoint,
        LightKind::Sun => Icon::LightSun,
        LightKind::Spot => Icon::LightSpot,
        LightKind::Area => Icon::LightArea,
    }
}

/// The row whose camera data icon is the active one (`icon_bgcolor[3] = 0.2f`): the scene camera, the first camera.
fn is_scene_camera(scene: &Scene, id: u32) -> bool {
    scene.objects.iter().find(|o| matches!(o.data, ObjectData::Camera)).is_some_and(|o| o.id == id)
}

/// Left edge of the icon slot of a row at `level`.
fn icon_x(user: &UserDef, region: Rcti, level: i32) -> i32 {
    region.xmin + level * user.widget_unit + user.widget_unit
}

/// Bottom of row `i` in window coordinates, `None` below the region.
fn row_ymin(user: &UserDef, region: Rcti, i: usize) -> Option<i32> {
    let rh = region.size_y() + 1;
    let starty = -OL_Y_OFFSET - user.widget_unit * (i as i32 + 1);
    (starty >= -rh).then_some(region.ymin + rh + starty)
}

/// The slots of `child_icons()` of row `i`, which ends its name `name_width` pixels after the name start.
pub fn layout(user: &UserDef, region: Rcti, level: i32, i: usize, name_width: i32, icons: &[Icon]) -> Vec<(Icon, Rcti)> {
    let Some(ymin) = row_ymin(user, region, i) else { return Vec::new() };
    let unit = user.widget_unit;
    let ufac = unit as f32 / 20.0;
    let name_x = region.xmin + level * unit + (2.0 * unit as f32 + 4.0 * ufac) as i32;
    let x = name_x + unit + name_width;
    icons
        .iter()
        .enumerate()
        .map(|(k, &icon)| {
            let xmin = x + k as i32 * unit;
            (icon, Rcti { xmin, xmax: xmin + unit - 1, ymin, ymax: ymin + unit - 1 })
        })
        .collect()
}

/// Rows whose icon gets the active circle: the active layer collection and the active object.
pub fn active_rows(scene: &Scene) -> Vec<usize> {
    scene
        .outliner_rows()
        .iter()
        .enumerate()
        .filter(|(_, r)| match r.kind {
            RowKind::ActiveCollection => true,
            RowKind::Object(id) => scene.active == Some(id),
            _ => false,
        })
        .map(|(i, _)| i)
        .collect()
}

/// A box with corner radius `UI_UNIT_Y / 4`: `TH_TEXT` at alpha 0.4 inside, 0.2 on the 1 px border.
fn draw_indicator(buf: &mut [u32], width: usize, height: usize, user: &UserDef, slot: Rcti, inner: f32) {
    let unit = user.widget_unit as f32;
    let ufac = (unit / 20.0).round() as i32;
    let (x0, x1) = (slot.xmin, slot.xmax + 1);
    let (y0, y1) = (slot.ymin + ufac, slot.ymax + 1 - ufac);
    let radius = unit / 4.0;
    let line = user.pixelsize.max(1) as f32;
    let (cx, cy) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
    let (hx, hy) = ((x1 - x0) as f32 / 2.0, (y1 - y0) as f32 / 2.0);
    let text = theme::SPACE_OUTLINER_TEXT;
    for y in y0.max(0)..y1.min(height as i32) {
        for x in x0.max(0)..x1.min(width as i32) {
            let qx = (x as f32 + 0.5 - cx).abs() - (hx - radius);
            let qy = (y as f32 + 0.5 - cy).abs() - (hy - radius);
            let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius;
            let cover = (0.5 - d).clamp(0.0, 1.0);
            if cover <= 0.0 {
                continue;
            }
            let alpha = if d > -line { 0.2 } else { inner };
            let p = &mut buf[(height - 1 - y as usize) * width + x as usize];
            *p = blend_px(*p, text, alpha * cover);
        }
    }
}

/// The active circles; drawn before the icons.
pub fn draw_active(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti, scene: &Scene) {
    let rows = scene.outliner_rows();
    for i in active_rows(scene) {
        let Some(ymin) = row_ymin(user, region, i) else { continue };
        let xmin = icon_x(user, region, rows[i].level);
        let unit = user.widget_unit;
        draw_indicator(buf, width, height, user, Rcti { xmin, xmax: xmin + unit - 1, ymin, ymax: ymin + unit - 1 }, 0.4);
    }
}

/// Draws the icon rows of all object rows (`alpha_fac * 0.8`, theme coloured).
pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti, scene: &Scene) {
    for (i, row) in scene.outliner_rows().iter().enumerate() {
        let RowKind::Object(id) = row.kind else { continue };
        let Some(object) = scene.index_of(id).map(|k| &scene.objects[k]) else { continue };
        let name_width = font::string_width(user, &row.name);
        for (icon, slot) in layout(user, region, row.level, i, name_width, &child_icons(object)) {
            if slot.xmax > region.xmax {
                break;
            }
            if icon == Icon::CameraData && is_scene_camera(scene, id) {
                draw_indicator(buf, width, height, user, slot, 0.2);
            }
            let c = icon_theme::theme_color(icon).unwrap_or(theme::SPACE_OUTLINER_TEXT);
            draw_icon(buf, width, height, user, slot, icon, Rgba { a: (c.a as f32 * 0.8) as u8, ..c });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_rows_get_data_icons() {
        let s = Scene::startup();
        let of = |n: &str| child_icons(s.objects.iter().find(|o| o.name == n).unwrap());
        assert_eq!(of("Cube"), [Icon::MeshData]);
        assert_eq!(of("Camera"), [Icon::CameraData]);
        assert_eq!(of("Light"), [Icon::LightPoint]);
    }

    #[test]
    fn modifier_wrench_comes_before_the_data() {
        let mut s = Scene::startup();
        let k = s.objects.iter().position(|o| o.name == "Cube").unwrap();
        s.objects[k].subsurf = 1;
        assert_eq!(child_icons(&s.objects[k]), [Icon::ModWrench, Icon::MeshData]);
    }

    /* 1x, 200x163 region: Cube is row 3 (y 81..=100), level 2, name x 84; 28 px of name => icons from 132. */
    #[test]
    fn slots_follow_the_name() {
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 162 };
        let l = layout(&user, region, 2, 3, 28, &[Icon::ModWrench, Icon::MeshData]);
        assert_eq!(l[0].1, Rcti { xmin: 132, xmax: 151, ymin: 81, ymax: 100 });
        assert_eq!(l[1].1.xmin, 152);
    }

    #[test]
    fn collection_and_active_object_are_circled() {
        assert_eq!(active_rows(&Scene::startup()), [1, 3]);
    }
}
