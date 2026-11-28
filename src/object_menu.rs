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

/// `VIEW3D_MT_object_parent`: `operator_enum("object.parent_set", "type")` lists `prop_make_parent_types` and
/// `operator_enum("object.parent_clear", "type")` lists `prop_clear_parent_types` (object_relations.cc). The
/// armature sub-items keep the leading spaces of their names.
pub const PARENT: &[Row] = &[
    op("object.parent_set", "Object", &[("type", "OBJECT")]),
    op("object.parent_set", "Armature Deform", &[("type", "ARMATURE")]),
    op("object.parent_set", "   With Empty Groups", &[("type", "ARMATURE_NAME")]),
    op("object.parent_set", "   With Automatic Weights", &[("type", "ARMATURE_AUTO")]),
    op("object.parent_set", "   With Envelope Weights", &[("type", "ARMATURE_ENVELOPE")]),
    op("object.parent_set", "Bone", &[("type", "BONE")]),
    op("object.parent_set", "Bone Relative", &[("type", "BONE_RELATIVE")]),
    op("object.parent_set", "Curve Deform", &[("type", "CURVE")]),
    op("object.parent_set", "Follow Path", &[("type", "FOLLOW")]),
    op("object.parent_set", "Path Constraint", &[("type", "PATH_CONST")]),
    op("object.parent_set", "Lattice Deform", &[("type", "LATTICE")]),
    op("object.parent_set", "Vertex", &[("type", "VERTEX")]),
    op("object.parent_set", "Vertex (Triangle)", &[("type", "VERTEX_TRI")]),
    Row::Separator,
    op("object.parent_no_inverse_set", "Make Parent without Inverse", &[("keep_transform", "False")]),
    op("object.parent_no_inverse_set", "Make Parent without Inverse (Keep Transform)", &[("keep_transform", "True")]),
    Row::Separator,
    op("object.parent_clear", "Clear Parent", &[("type", "CLEAR")]),
    op("object.parent_clear", "Clear and Keep Transformation", &[("type", "CLEAR_KEEP_TRANSFORM")]),
    op("object.parent_clear", "Clear Parent Inverse", &[("type", "CLEAR_INVERSE")]),
];

/// `VIEW3D_MT_object_showhide`; the first row is `layout.operator("object.hide_view_clear")` with `ot->name`
/// (object_edit.cc), the others carry the `text=` of the call.
pub const SHOWHIDE: &[Row] = &[
    op("object.hide_view_clear", "Show Hidden Objects", &[]),
    Row::Separator,
    op("object.hide_view_set", "Hide Selected", &[("unselected", "False")]),
    op("object.hide_view_set", "Hide Unselected", &[("unselected", "True")]),
];

/// `VIEW3D_MT_snap` (space_view3d.py): the `text=` of every `layout.operator` call.
pub const SNAP: &[Row] = &[
    op("view3d.snap_selected_to_grid", "Selection to Grid", &[]),
    op("view3d.snap_selected_to_cursor", "Selection to Cursor", &[("use_offset", "False")]),
    op("view3d.snap_selected_to_cursor", "Selection to Cursor (Keep Offset)", &[("use_offset", "True")]),
    op("view3d.snap_selected_to_active", "Selection to Active", &[]),
    Row::Separator,
    op("view3d.snap_cursor_to_selected", "Cursor to Selected", &[]),
    op("view3d.snap_cursor_to_center", "Cursor to World Origin", &[]),
    op("view3d.snap_cursor_to_grid", "Cursor to Grid", &[]),
    op("view3d.snap_cursor_to_active", "Cursor to Active", &[]),
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

    #[test]
    fn parent_enum_items_match_object_relations_cc() {
        let types = |id: &str| -> Vec<&'static str> {
            PARENT
                .iter()
                .filter_map(|r| match r {
                    Row::Operator { idname, props, .. } if *idname == id => props.iter().find(|(k, _)| *k == "type").map(|p| p.1),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(
            types("object.parent_set"),
            [
                "OBJECT", "ARMATURE", "ARMATURE_NAME", "ARMATURE_AUTO", "ARMATURE_ENVELOPE", "BONE", "BONE_RELATIVE", "CURVE",
                "FOLLOW", "PATH_CONST", "LATTICE", "VERTEX", "VERTEX_TRI",
            ]
        );
        assert_eq!(types("object.parent_clear"), ["CLEAR", "CLEAR_KEEP_TRANSFORM", "CLEAR_INVERSE"]);
    }

    #[test]
    fn parent_menu_groups_and_keep_transform_props() {
        let l = labels_of(PARENT);
        assert_eq!(l.iter().filter(|x| **x == "-").count(), 2);
        assert_eq!(l[2], "   With Empty Groups");
        assert_eq!(&l[14..16], ["Make Parent without Inverse", "Make Parent without Inverse (Keep Transform)"]);
        assert!(matches!(PARENT[14], Row::Operator { props: [("keep_transform", "False")], .. }));
        assert!(matches!(PARENT[15], Row::Operator { props: [("keep_transform", "True")], .. }));
        assert_eq!(&l[17..], ["Clear Parent", "Clear and Keep Transformation", "Clear Parent Inverse"]);
    }
}
