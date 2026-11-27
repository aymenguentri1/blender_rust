//! Icons drawn from Blender's own artwork.
//!
//! Blender draws its icons (`ICON_*`, `UI_icon_draw()` in editors/interface/interface_icons.cc) from the
//! `release/datafiles/icons_svg/*.svg` files. `scripts/gen_icon_masks.py` renders each of those SVGs to a 64 x 64
//! alpha mask (`assets/icon_masks.bin`, names in `assets/icon_masks.txt`); `draw_icon` scales a mask to
//! `ICON_DEFAULT_HEIGHT` (16 px) x `UI_SCALE_FAC` with a 4 x 4 box filter and tints it with the given colour.
//! Deviation: the icons are single coloured (the theme's icon colours are not applied) and `LeftArrow` is the
//! mirrored `ICON_DISCLOSURE_TRI_RIGHT`.

use crate::area::Rcti;
use crate::theme::Rgba;
use crate::userdef::UserDef;
use crate::widget_draw::blend_px;

/// Size of an icon in icon units (`ICON_DEFAULT_HEIGHT`).
pub const GRID: f32 = 16.0;

/// Side of the stored masks in pixels.
const MASK: usize = 64;
const MASK_NAMES: &str = include_str!("../assets/icon_masks.txt");
const MASK_DATA: &[u8] = include_bytes!("../assets/icon_masks.bin");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// `ICON_VIEW3D`, the 3D viewport editor type.
    View3D,
    /// `ICON_OBJECT_DATAMODE`.
    ObjectMode,
    /// `ICON_SCULPTMODE_HLT`: a brush.
    SculptMode,
    /// `ICON_DOWNARROW_HLT`.
    DownArrow,
    /// `ICON_ORIENTATION_GLOBAL`.
    OrientationGlobal,
    /// `ICON_PIVOT_MEDIAN`, `ICON_PIVOT_CURSOR`, `ICON_PIVOT_INDIVIDUAL`.
    PivotMedian,
    PivotCursor,
    PivotIndividual,
    /// `ICON_SNAP_OFF` (the magnet).
    Snap,
    /// `ICON_PROP_OFF`.
    Proportional,
    /// `ICON_OVERLAY`.
    Overlays,
    /// `ICON_XRAY`.
    Xray,
    /// `ICON_SHADING_WIRE`, `ICON_SHADING_SOLID`, `ICON_MATERIAL`, `ICON_SHADING_RENDERED`.
    ShadingWire,
    ShadingSolid,
    ShadingMaterial,
    ShadingRendered,
    /// `ICON_DISCLOSURE_TRI_RIGHT`: the arrow of a collapsed panel (`DownArrow` is the expanded one).
    RightArrow,
    /// `ICON_DISCLOSURE_TRI_RIGHT` mirrored: the left end of a number field.
    LeftArrow,
    /// `ICON_RESTRICT_VIEW_OFF`: the monitor of a modifier's "Show in Viewport" toggle.
    Monitor,
    /// `ICON_RESTRICT_RENDER_OFF`: the camera of a modifier's "Show in Render" toggle.
    Camera,
    /// `ICON_HIDE_OFF`: the eye of the outliner's hide column.
    Eye,
    /// `ICON_GRIP`: the six dots of a modifier panel's drag handle.
    Grip,
    /// `ICON_EDITMODE_HLT`: the square with corner vertices of a modifier's "Show in Edit Mode" toggle.
    EditMode,
    /// The modifier type icons of the panel header (`ICON_MOD_*`); `ModWrench` is the generic `ICON_MODIFIER`.
    ModWrench,
    ModArray,
    ModMirror,
    ModSolidify,
    ModBevel,
    ModSmooth,
    ModDeform,
    ModTriangulate,
    ModBoolean,
    ModWireframe,
    ModDecimate,
    ModRemesh,
    /// `ICON_OUTLINER_COLLECTION`: the open box of the outliner's collection rows.
    OutlinerCollection,
    /// `ICON_OUTLINER_OB_MESH`, `ICON_OUTLINER_OB_CAMERA`, `ICON_OUTLINER_OB_LIGHT`, `ICON_OUTLINER_OB_EMPTY`: the
    /// object type icons of `icon_from_object_type()`.
    ObMesh,
    ObCamera,
    ObLight,
    ObEmpty,
    /// The icons of the Add menu items (`VIEW3D_MT_mesh_add`, `VIEW3D_MT_empty_add`, `VIEW3D_MT_light_add`):
    /// `ICON_MESH_PLANE` ... `ICON_MESH_GRID`.
    MeshPlane,
    MeshCube,
    MeshCircle,
    MeshUvSphere,
    MeshIcoSphere,
    MeshCylinder,
    MeshCone,
    MeshTorus,
    MeshGrid,
    /// `ICON_EMPTY_AXIS`, `ICON_EMPTY_ARROWS`, `ICON_EMPTY_SINGLE_ARROW`, `ICON_CUBE`, `ICON_SPHERE`, `ICON_CONE`.
    EmptyAxis,
    EmptyArrows,
    EmptySingleArrow,
    Cube,
    Sphere,
    Cone,
    /// `ICON_LIGHT_POINT`, `ICON_LIGHT_SUN`, `ICON_LIGHT_SPOT`, `ICON_LIGHT_AREA`.
    LightPoint,
    LightSun,
    LightSpot,
    LightArea,
    /// `ICON_RIGHTARROW`: the thin chevron of a collapsed outliner row (`ICON_DOWNARROW_HLT` is the open one).
    ChevronRight,
}

impl Icon {
    pub const ALL: &'static [Icon] = &[Icon::View3D, Icon::ObjectMode, Icon::SculptMode, Icon::DownArrow, Icon::OrientationGlobal, Icon::PivotMedian, Icon::PivotCursor, Icon::PivotIndividual, Icon::Snap, Icon::Proportional, Icon::Overlays, Icon::Xray, Icon::ShadingWire, Icon::ShadingSolid, Icon::ShadingMaterial, Icon::ShadingRendered, Icon::RightArrow, Icon::LeftArrow, Icon::Monitor, Icon::Camera, Icon::Eye, Icon::Grip, Icon::EditMode, Icon::ModWrench, Icon::ModArray, Icon::ModMirror, Icon::ModSolidify, Icon::ModBevel, Icon::ModSmooth, Icon::ModDeform, Icon::ModTriangulate, Icon::ModBoolean, Icon::ModWireframe, Icon::ModDecimate, Icon::ModRemesh, Icon::OutlinerCollection, Icon::ObMesh, Icon::ObCamera, Icon::ObLight, Icon::ObEmpty, Icon::MeshPlane, Icon::MeshCube, Icon::MeshCircle, Icon::MeshUvSphere, Icon::MeshIcoSphere, Icon::MeshCylinder, Icon::MeshCone, Icon::MeshTorus, Icon::MeshGrid, Icon::EmptyAxis, Icon::EmptyArrows, Icon::EmptySingleArrow, Icon::Cube, Icon::Sphere, Icon::Cone, Icon::LightPoint, Icon::LightSun, Icon::LightSpot, Icon::LightArea, Icon::ChevronRight];

    /// The file name (without `.svg`) of the icon in `release/datafiles/icons_svg`.
    pub fn svg_name(self) -> &'static str {
        match self {
            Icon::View3D => "view3d",
            Icon::ObjectMode => "object_datamode",
            Icon::SculptMode => "sculptmode_hlt",
            Icon::DownArrow => "downarrow_hlt",
            Icon::OrientationGlobal => "orientation_global",
            Icon::PivotMedian => "pivot_median",
            Icon::PivotCursor => "pivot_cursor",
            Icon::PivotIndividual => "pivot_individual",
            Icon::Snap => "snap_off",
            Icon::Proportional => "prop_off",
            Icon::Overlays => "overlay",
            Icon::Xray => "xray",
            Icon::ShadingWire => "shading_wire",
            Icon::ShadingSolid => "shading_solid",
            Icon::ShadingMaterial => "material",
            Icon::ShadingRendered => "shading_rendered",
            Icon::RightArrow => "disclosure_tri_right",
            Icon::LeftArrow => "disclosure_tri_right",
            Icon::Monitor => "restrict_view_off",
            Icon::Camera => "restrict_render_off",
            Icon::Eye => "hide_off",
            Icon::Grip => "grip",
            Icon::EditMode => "editmode_hlt",
            Icon::ModWrench => "modifier",
            Icon::ModArray => "mod_array",
            Icon::ModMirror => "mod_mirror",
            Icon::ModSolidify => "mod_solidify",
            Icon::ModBevel => "mod_bevel",
            Icon::ModSmooth => "mod_smooth",
            Icon::ModDeform => "mod_simpledeform",
            Icon::ModTriangulate => "mod_triangulate",
            Icon::ModBoolean => "mod_boolean",
            Icon::ModWireframe => "mod_wireframe",
            Icon::ModDecimate => "mod_decim",
            Icon::ModRemesh => "mod_remesh",
            Icon::OutlinerCollection => "outliner_collection",
            Icon::ObMesh => "outliner_ob_mesh",
            Icon::ObCamera => "outliner_ob_camera",
            Icon::ObLight => "outliner_ob_light",
            Icon::ObEmpty => "outliner_ob_empty",
            Icon::MeshPlane => "mesh_plane",
            Icon::MeshCube => "mesh_cube",
            Icon::MeshCircle => "mesh_circle",
            Icon::MeshUvSphere => "mesh_uvsphere",
            Icon::MeshIcoSphere => "mesh_icosphere",
            Icon::MeshCylinder => "mesh_cylinder",
            Icon::MeshCone => "mesh_cone",
            Icon::MeshTorus => "mesh_torus",
            Icon::MeshGrid => "mesh_grid",
            Icon::EmptyAxis => "empty_axis",
            Icon::EmptyArrows => "empty_arrows",
            Icon::EmptySingleArrow => "empty_single_arrow",
            Icon::Cube => "cube",
            Icon::Sphere => "sphere",
            Icon::Cone => "cone",
            Icon::LightPoint => "light_point",
            Icon::LightSun => "light_sun",
            Icon::LightSpot => "light_spot",
            Icon::LightArea => "light_area",
            Icon::ChevronRight => "rightarrow",
        }
    }

    fn mask(self) -> &'static [u8] {
        let k = MASK_NAMES.lines().position(|n| n == self.svg_name()).expect("icon without a mask");
        &MASK_DATA[k * MASK * MASK..(k + 1) * MASK * MASK]
    }
}

pub fn icon_size(user: &UserDef) -> i32 {
    ((GRID * user.scale_factor).round() as i32).max(1)
}

/// Draws `icon` centred in `rect` (window pixels, y up) with `color` (its alpha scales the mask).
pub fn draw_icon(buf: &mut [u32], width: usize, height: usize, user: &UserDef, rect: Rcti, icon: Icon, color: Rgba) {
    if width == 0 || height == 0 || buf.len() < width * height {
        return;
    }
    let size = icon_size(user);
    let x0 = rect.xmin + (rect.size_x() + 1 - size) / 2;
    let y0 = rect.ymin + (rect.size_y() + 1 - size) / 2;
    let mask = icon.mask();
    let flip = icon == Icon::LeftArrow;
    let base_a = color.a as f32 / 255.0;
    const N: usize = 4;
    let k = MASK as f32 / size as f32;
    for py in 0..size {
        let wy = y0 + (size - 1 - py); /* mask row 0 is the top, window y is up */
        if wy < 0 || wy >= height as i32 {
            continue;
        }
        let row = (height - 1 - wy as usize) * width;
        for px in 0..size {
            let wx = x0 + px;
            if wx < 0 || wx >= width as i32 {
                continue;
            }
            let mut sum = 0u32;
            for sy in 0..N {
                let my = (((py as f32) + (sy as f32 + 0.5) / N as f32) * k) as usize;
                for sx in 0..N {
                    let mut mx = (((px as f32) + (sx as f32 + 0.5) / N as f32) * k) as usize;
                    if flip {
                        mx = MASK - 1 - mx;
                    }
                    sum += mask[my.min(MASK - 1) * MASK + mx.min(MASK - 1)] as u32;
                }
            }
            let a = sum as f32 / (255.0 * (N * N) as f32) * base_a;
            if a > 0.0 {
                let px = &mut buf[row + wx as usize];
                *px = blend_px(*px, color, a);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bg() -> u32 {
        0x0020_2020
    }

    /* A solid sphere fills its centre and leaves the corner of the icon box alone. */
    #[test]
    fn solid_sphere_covers_centre_not_corner() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (20usize, 20usize);
        let mut buf = vec![bg(); w * h];
        let rect = Rcti { xmin: 0, xmax: 19, ymin: 0, ymax: 19 };
        draw_icon(&mut buf, w, h, &user, rect, Icon::ShadingSolid, Rgba::hex(0xffffffff));
        assert_ne!(buf[10 * w + 10], bg());
        assert_eq!(buf[0], bg());
    }

    /* A zero alpha colour draws nothing. */
    #[test]
    fn transparent_colour_draws_nothing() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (20usize, 20usize);
        let mut buf = vec![bg(); w * h];
        let rect = Rcti { xmin: 0, xmax: 19, ymin: 0, ymax: 19 };
        draw_icon(&mut buf, w, h, &user, rect, Icon::View3D, Rgba::hex(0xffffff00));
        assert!(buf.iter().all(|&p| p == bg()));
    }

    /* Every icon draws something. */
    #[test]
    fn every_icon_draws_pixels() {
        let user = UserDef::from_window_scale(1.0);
        let rect = Rcti { xmin: 0, xmax: 19, ymin: 0, ymax: 19 };
        for &icon in Icon::ALL {
            let mut buf = vec![bg(); 400];
            draw_icon(&mut buf, 20, 20, &user, rect, icon, Rgba::hex(0xffffffff));
            assert!(buf.iter().any(|&p| p != bg()), "{icon:?}");
        }
    }

    /* The mask file holds one 64 x 64 mask per name, and every icon has a name in it. */
    #[test]
    fn the_mask_file_matches_the_name_list() {
        assert_eq!(MASK_DATA.len(), MASK_NAMES.lines().count() * MASK * MASK);
        for &icon in Icon::ALL {
            assert!(MASK_NAMES.lines().any(|n| n == icon.svg_name()), "{icon:?}");
            assert!(icon.mask().iter().any(|&a| a > 60), "{icon:?} mask is empty");
        }
    }

    /* The mirrored arrow draws its mass on the other side of the box than the right arrow. */
    #[test]
    fn left_arrow_mirrors_right_arrow() {
        let user = UserDef::from_window_scale(1.0);
        let rect = Rcti { xmin: 0, xmax: 19, ymin: 0, ymax: 19 };
        let (mut l, mut r) = (vec![bg(); 400], vec![bg(); 400]);
        draw_icon(&mut l, 20, 20, &user, rect, Icon::LeftArrow, Rgba::hex(0xffffffff));
        draw_icon(&mut r, 20, 20, &user, rect, Icon::RightArrow, Rgba::hex(0xffffffff));
        for y in 0..20 {
            for x in 0..20 {
                assert_eq!(l[y * 20 + x], r[y * 20 + 19 - x]);
            }
        }
    }

    /* The chevron's svg canvas is 7 x 11 design pixels: it keeps that shape (taller than wide), centred. */
    #[test]
    fn small_icons_are_not_stretched_to_the_grid() {
        let m = Icon::ChevronRight.mask();
        let cols: Vec<usize> = (0..MASK).filter(|&x| (0..MASK).any(|y| m[y * MASK + x] > 0)).collect();
        let rows: Vec<usize> = (0..MASK).filter(|&y| (0..MASK).any(|x| m[y * MASK + x] > 0)).collect();
        let (w, h) = (cols.last().unwrap() - cols[0] + 1, rows.last().unwrap() - rows[0] + 1);
        assert!(w < h, "{w}x{h}");
        assert!(w <= 7 * MASK / 16 && h <= 11 * MASK / 16 + 1);
    }
}
