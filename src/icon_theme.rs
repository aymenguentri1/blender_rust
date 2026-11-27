//! Theme colours of icons.
//!
//! Ported from Blender:
//!  - `DEF_ICON_SCENE` ... `DEF_ICON_SHADING` (editors/include/UI_icons.hh): the colour class of each icon
//!  - `get_icon_color_4ubv()` (editors/interface/resources.cc:1462): the classes are only coloured in the
//!    outliner window and the properties navigation bar, everywhere else icons stay monochrome
//!  - `.tui.icon_scene` ... `.icon_shading` (release/datafiles/userdef/userdef_default_theme.c:275-280)

use crate::icons::Icon;
use crate::theme::Rgba;

pub const SCENE: Rgba = Rgba::hex(0xccccccff);
pub const OBJECT: Rgba = Rgba::hex(0xe19658ff);
pub const OBJECT_DATA: Rgba = Rgba::hex(0x00d4a3ff);
pub const MODIFIER: Rgba = Rgba::hex(0x74a2ffff);
pub const SHADING: Rgba = Rgba::hex(0xcc6670ff);

/// The colour of `icon` where theme icon colours apply; `None` for the monochrome ones.
pub fn theme_color(icon: Icon) -> Option<Rgba> {
    use Icon::*;
    match icon {
        ToolSettings | TabScene | Output | SceneData | RenderResult => Some(SCENE),
        ObMesh | ObCamera | ObLight | ObEmpty | ObjectData => Some(OBJECT),
        MeshData | CameraData | LightData | EmptyData | LightPoint | LightSun | LightSpot | LightArea => Some(OBJECT_DATA),
        ModWrench | ModArray | ModMirror | ModSolidify | ModBevel | ModSmooth | ModDeform | ModTriangulate | ModBoolean
        | ModWireframe | ModDecimate | ModRemesh | Constraint | Physics => Some(MODIFIER),
        ShadingMaterial | World => Some(SHADING),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_follow_ui_icons() {
        assert_eq!(theme_color(Icon::ObMesh), Some(OBJECT));
        assert_eq!(theme_color(Icon::MeshData), Some(OBJECT_DATA));
        assert_eq!(theme_color(Icon::ModWrench), Some(MODIFIER));
        assert_eq!(theme_color(Icon::World), Some(SHADING));
        assert_eq!(theme_color(Icon::Output), Some(SCENE));
        assert_eq!(theme_color(Icon::OutlinerCollection), None);
        assert_eq!(theme_color(Icon::Eye), None);
    }
}
