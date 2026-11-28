//! VIEW3D_MT_object: the Object menu of the 3D viewport header in Object Mode, as pure layout rows.
//!
//! Ported from `scripts/startup/bl_ui/space_view3d.py` (`class VIEW3D_MT_object`, `draw`). Submenu labels are the
//! `bl_label` of their class, operator labels the `text=` of the call or else `ot->name` (object_add.cc,
//! object_edit.cc, object_ops.cc). `operator_menu_enum` (Set Origin) is an operator row here.

use crate::sculpt_menu::Row;

const fn op(idname: &'static str, label: &'static str, props: &'static [(&'static str, &'static str)]) -> Row {
    Row::Operator { idname, label, props, checkbox: None }
}

const fn menu(idname: &'static str, label: &'static str) -> Row {
    Row::Menu { idname, label }
}

/// `ob.type in {'MESH', 'CURVE', 'FONT'}` of `context.object`, the condition of Shade Auto Smooth.
pub fn auto_smooth_type(ob_type: Option<&str>) -> bool {
    matches!(ob_type, Some("MESH" | "CURVE" | "FONT"))
}

/// Rows of the menu; `ob_type` is the RNA type of the active object, if any.
pub fn rows(ob_type: Option<&str>) -> Vec<Row> {
    use Row::Separator as Sep;
    let mut r = vec![
        menu("VIEW3D_MT_transform_object", "Transform"),
        op("object.origin_set", "Set Origin", &[]),
        menu("VIEW3D_MT_mirror", "Mirror"),
        menu("VIEW3D_MT_object_clear", "Clear"),
        menu("VIEW3D_MT_object_apply", "Apply"),
        menu("VIEW3D_MT_snap", "Snap"),
        Sep,
        op("object.duplicate_move", "Duplicate Objects", &[]),
        op("object.duplicate_move_linked", "Duplicate Linked", &[]),
        op("object.join", "Join", &[]),
        Sep,
        op("view3d.copybuffer", "Copy Objects", &[]),
        op("view3d.pastebuffer", "Paste Objects", &[]),
        Sep,
        menu("VIEW3D_MT_object_asset", "Asset"),
        menu("VIEW3D_MT_object_collection", "Collection"),
        Sep,
        menu("VIEW3D_MT_object_liboverride", "Library Override"),
        menu("VIEW3D_MT_object_relations", "Relations"),
        menu("VIEW3D_MT_object_parent", "Parent"),
        menu("VIEW3D_MT_object_modifiers", "Modifiers"),
        menu("VIEW3D_MT_object_constraints", "Constraints"),
        menu("VIEW3D_MT_object_track", "Track"),
        menu("VIEW3D_MT_make_links", "Link/Transfer Data"),
        Sep,
        op("object.shade_smooth", "Shade Smooth", &[]),
    ];
    if auto_smooth_type(ob_type) {
        r.push(op("object.shade_auto_smooth", "Shade Auto Smooth", &[]));
    }
    r.extend([
        op("object.shade_flat", "Shade Flat", &[]),
        Sep,
        menu("VIEW3D_MT_object_animation", "Animation"),
        menu("VIEW3D_MT_object_rigid_body", "Rigid Body"),
        Sep,
        menu("VIEW3D_MT_object_quick_effects", "Quick Effects"),
        Sep,
        menu("VIEW3D_MT_object_convert", "Convert"),
        Sep,
        menu("VIEW3D_MT_object_showhide", "Show/Hide"),
        menu("VIEW3D_MT_object_cleanup", "Clean Up"),
        Sep,
        op("object.delete", "Delete Global", &[("use_global", "True")]),
        op("object.delete", "Delete", &[("use_global", "False")]),
    ]);
    r
}

/// `VIEW3D_MT_object_clear`.
pub const CLEAR: &[Row] = &[
    op("object.location_clear", "Location", &[("clear_delta", "False")]),
    op("object.rotation_clear", "Rotation", &[("clear_delta", "False")]),
    op("object.scale_clear", "Scale", &[("clear_delta", "False")]),
    Row::Separator,
    op("object.origin_clear", "Origin", &[]),
];

/// `VIEW3D_MT_object_apply`: `transform_apply` rows set (location, rotation, scale) as the menu does; the
/// `transforms_to_deltas` rows use the `mode` enum of `TransformsToDeltas` (bl_operators/object.py).
pub const APPLY: &[Row] = &[
    op("object.transform_apply", "Location", &[("location", "True"), ("rotation", "False"), ("scale", "False")]),
    op("object.transform_apply", "Rotation", &[("location", "False"), ("rotation", "True"), ("scale", "False")]),
    op("object.transform_apply", "Scale", &[("location", "False"), ("rotation", "False"), ("scale", "True")]),
    op("object.transform_apply", "All Transforms", &[("location", "True"), ("rotation", "True"), ("scale", "True")]),
    op("object.transform_apply", "Rotation & Scale", &[("location", "False"), ("rotation", "True"), ("scale", "True")]),
    Row::Separator,
    op("object.transforms_to_deltas", "Location to Deltas", &[("mode", "LOC")]),
    op("object.transforms_to_deltas", "Rotation to Deltas", &[("mode", "ROT")]),
    op("object.transforms_to_deltas", "Scale to Deltas", &[("mode", "SCALE")]),
    op("object.transforms_to_deltas", "All Transforms to Deltas", &[("mode", "ALL")]),
    op("object.anim_transforms_to_deltas", "Animated Transforms to Deltas", &[]),
    Row::Separator,
    op("object.visual_transform_apply", "Visual Transform", &[]),
    op("object.convert", "Visual Geometry to Mesh", &[("target", "MESH")]),
    op("object.visual_geometry_to_objects", "Visual Geometry to Objects", &[]),
    op("object.duplicates_make_real", "Make Instances Real", &[]),
    op("object.parent_inverse_apply", "Parent Inverse", &[]),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(ob: Option<&str>) -> Vec<&'static str> {
        rows(ob)
            .iter()
            .map(|r| match r {
                Row::Separator => "-",
                Row::Menu { label, .. } | Row::Operator { label, .. } => label,
            })
            .collect()
    }

    #[test]
    fn labels_in_blender_order() {
        assert_eq!(
            labels(Some("MESH")),
            [
                "Transform", "Set Origin", "Mirror", "Clear", "Apply", "Snap", "-", "Duplicate Objects",
                "Duplicate Linked", "Join", "-", "Copy Objects", "Paste Objects", "-", "Asset", "Collection", "-",
                "Library Override", "Relations", "Parent", "Modifiers", "Constraints", "Track", "Link/Transfer Data", "-",
                "Shade Smooth", "Shade Auto Smooth", "Shade Flat", "-", "Animation", "Rigid Body", "-", "Quick Effects",
                "-", "Convert", "-", "Show/Hide", "Clean Up", "-", "Delete Global", "Delete",
            ]
        );
    }

    #[test]
    fn shade_auto_smooth_only_for_mesh_curve_and_font() {
        for t in ["MESH", "CURVE", "FONT"] {
            assert!(labels(Some(t)).contains(&"Shade Auto Smooth"), "{t}");
        }
        for t in [Some("EMPTY"), Some("CAMERA"), None] {
            assert!(!labels(t).contains(&"Shade Auto Smooth"), "{t:?}");
        }
        assert_eq!(labels(Some("MESH")).len(), labels(None).len() + 1);
    }

    #[test]
    fn delete_rows_set_use_global() {
        let r = rows(None);
        let d: Vec<_> = r
            .iter()
            .filter_map(|x| match x {
                Row::Operator { idname: "object.delete", props, .. } => Some(*props),
                _ => None,
            })
            .collect();
        assert_eq!(d, [&[("use_global", "True")][..], &[("use_global", "False")][..]]);
    }

    fn labels_of(rows: &[Row]) -> Vec<&'static str> {
        rows.iter()
            .map(|r| match r {
                Row::Separator => "-",
                Row::Menu { label, .. } | Row::Operator { label, .. } => label,
            })
            .collect()
    }

    #[test]
    fn clear_rows_clear_the_transform_not_the_delta() {
        assert_eq!(labels_of(CLEAR), ["Location", "Rotation", "Scale", "-", "Origin"]);
        assert!(CLEAR[..3].iter().all(|r| matches!(r, Row::Operator { props: [("clear_delta", "False")], .. })));
    }

    #[test]
    fn apply_rows_in_blender_order() {
        assert_eq!(
            labels_of(APPLY),
            [
                "Location", "Rotation", "Scale", "All Transforms", "Rotation & Scale", "-", "Location to Deltas",
                "Rotation to Deltas", "Scale to Deltas", "All Transforms to Deltas", "Animated Transforms to Deltas", "-",
                "Visual Transform", "Visual Geometry to Mesh", "Visual Geometry to Objects", "Make Instances Real",
                "Parent Inverse",
            ]
        );
    }

    #[test]
    fn transform_apply_bool_triples_match_the_menu() {
        let triples: Vec<_> = APPLY
            .iter()
            .filter_map(|r| match r {
                Row::Operator { idname: "object.transform_apply", props, .. } => {
                    Some(props.iter().map(|(_, v)| *v == "True").collect::<Vec<_>>())
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            triples,
            [vec![true, false, false], vec![false, true, false], vec![false, false, true], vec![true, true, true], vec![false, true, true]]
        );
        assert!(matches!(APPLY[13], Row::Operator { props: [("target", "MESH")], .. }));
    }
}
