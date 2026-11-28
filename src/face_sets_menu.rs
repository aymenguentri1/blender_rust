//! VIEW3D_MT_face_sets and VIEW3D_MT_face_sets_init: the Face Sets menus of the sculpt 3D viewport header,
//! as pure layout rows.
//!
//! Ported from `scripts/startup/bl_ui/space_view3d.py` (`class VIEW3D_MT_face_sets`, `class
//! VIEW3D_MT_face_sets_init`, `draw`). The trailing `template_node_operator_asset_menu_items` is not ported.

use crate::sculpt_menu::Row;

const fn op(
    idname: &'static str,
    label: &'static str,
    props: &'static [(&'static str, &'static str)],
) -> Row {
    Row::Operator {
        idname,
        label,
        props,
        checkbox: None,
    }
}

pub const FACE_SETS: &[Row] = &[
    op(
        "sculpt.face_sets_create",
        "Face Set from Masked",
        &[("mode", "MASKED")],
    ),
    op(
        "sculpt.face_sets_create",
        "Face Set from Visible",
        &[("mode", "VISIBLE")],
    ),
    op(
        "sculpt.face_sets_create",
        "Face Set from Edit Mode Selection",
        &[("mode", "SELECTION")],
    ),
    Row::Separator,
    Row::Menu {
        idname: "VIEW3D_MT_face_sets_init",
        label: "Initialize Face Sets",
    },
    Row::Separator,
    op("sculpt.face_set_edit", "Grow Face Set", &[("mode", "GROW")]),
    op(
        "sculpt.face_set_edit",
        "Shrink Face Set",
        &[("mode", "SHRINK")],
    ),
    Row::Separator,
    op(
        "sculpt.expand",
        "Expand Face Set by Topology",
        &[
            ("target", "FACE_SETS"),
            ("falloff_type", "GEODESIC"),
            ("invert", "False"),
            ("use_mask_preserve", "False"),
            ("use_modify_active", "False"),
        ],
    ),
    op(
        "sculpt.expand",
        "Expand Active Face Set",
        &[
            ("target", "FACE_SETS"),
            ("falloff_type", "BOUNDARY_FACE_SET"),
            ("invert", "False"),
            ("use_mask_preserve", "False"),
            ("use_modify_active", "True"),
        ],
    ),
    Row::Separator,
    op("sculpt.face_set_extract", "Extract Face Set", &[]),
    Row::Separator,
    op("sculpt.face_sets_randomize_colors", "Randomize Colors", &[]),
];

pub const FACE_SETS_INIT: &[Row] = &[
    op(
        "sculpt.face_sets_init",
        "By Loose Parts",
        &[("mode", "LOOSE_PARTS")],
    ),
    op(
        "sculpt.face_sets_init",
        "By Face Set Boundaries",
        &[("mode", "FACE_SET_BOUNDARIES")],
    ),
    op(
        "sculpt.face_sets_init",
        "By Materials",
        &[("mode", "MATERIALS")],
    ),
    op(
        "sculpt.face_sets_init",
        "By Normals",
        &[("mode", "NORMALS")],
    ),
    op(
        "sculpt.face_sets_init",
        "By UV Seams",
        &[("mode", "UV_SEAMS")],
    ),
    op(
        "sculpt.face_sets_init",
        "By Edge Creases",
        &[("mode", "CREASES")],
    ),
    op(
        "sculpt.face_sets_init",
        "By Edge Bevel Weight",
        &[("mode", "BEVEL_WEIGHT")],
    ),
    op(
        "sculpt.face_sets_init",
        "By Sharp Edges",
        &[("mode", "SHARP_EDGES")],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn label(r: &Row) -> &'static str {
        match r {
            Row::Separator => "-",
            Row::Menu { label, .. } | Row::Operator { label, .. } => label,
        }
    }

    #[test]
    fn face_sets_rows() {
        let l: Vec<_> = FACE_SETS.iter().map(label).collect();
        assert_eq!(
            l,
            [
                "Face Set from Masked",
                "Face Set from Visible",
                "Face Set from Edit Mode Selection",
                "-",
                "Initialize Face Sets",
                "-",
                "Grow Face Set",
                "Shrink Face Set",
                "-",
                "Expand Face Set by Topology",
                "Expand Active Face Set",
                "-",
                "Extract Face Set",
                "-",
                "Randomize Colors"
            ]
        );
        match &FACE_SETS[10] {
            Row::Operator { props, .. } => {
                assert!(props.contains(&("falloff_type", "BOUNDARY_FACE_SET")));
                assert!(props.contains(&("use_modify_active", "True")));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn init_rows_have_modes() {
        let modes: Vec<_> = FACE_SETS_INIT
            .iter()
            .map(|r| match r {
                Row::Operator {
                    idname: "sculpt.face_sets_init",
                    props,
                    ..
                } => props[0].1,
                _ => panic!(),
            })
            .collect();
        assert_eq!(
            modes,
            [
                "LOOSE_PARTS",
                "FACE_SET_BOUNDARIES",
                "MATERIALS",
                "NORMALS",
                "UV_SEAMS",
                "CREASES",
                "BEVEL_WEIGHT",
                "SHARP_EDGES"
            ]
        );
        assert_eq!(label(&FACE_SETS_INIT[2]), "By Materials");
    }
}
